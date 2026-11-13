import Foundation
import UIKit

private typealias Reply = @convention(c) (UnsafeMutableRawPointer, Bool, UnsafeRawPointer, Int) -> Void
private typealias LoginReply = @convention(c) (UnsafeMutableRawPointer, Int32, UnsafeRawPointer?, Int) -> Void

@_silgen_name("listener_native_initialize")
private func initializeCore(_ bytes: UnsafeRawPointer, _ count: Int)
@_silgen_name("listener_native_request")
private func requestCore(_ bytes: UnsafeRawPointer, _ count: Int, _ context: UnsafeMutableRawPointer, _ reply: Reply)
@_silgen_name("listener_native_open_url")
private func openCoreURL(_ bytes: UnsafeRawPointer, _ count: Int)
@_silgen_name("listener_present_qq_login")
private func presentQQ(_ controller: UnsafeMutableRawPointer, _ bytes: UnsafeRawPointer, _ count: Int,
                       _ context: UnsafeMutableRawPointer, _ reply: LoginReply)
struct CoreError: Decodable, LocalizedError {
    let code: String
    let message: String
    var errorDescription: String? { message }
}

private struct CoreRequest: Encodable {
    struct Key: CodingKey {
        let stringValue: String
        var intValue: Int? { nil }
        init(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { return nil }
    }
    let command: String
    let arguments: [String: any Encodable]

    func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: Key.self)
        try values.encode(command, forKey: Key(stringValue: "command"))
        for (key, value) in arguments {
            try value.encode(to: values.superEncoder(forKey: Key(stringValue: key)))
        }
    }
}

private final class CoreResponse {
    let continuation: CheckedContinuation<Data, Error>
    init(_ continuation: CheckedContinuation<Data, Error>) { self.continuation = continuation }
}

private final class QQResponse {
    let continuation: CheckedContinuation<String?, Error>
    init(_ continuation: CheckedContinuation<String?, Error>) { self.continuation = continuation }
}

enum Core {
    static func start() throws {
        let support = try FileManager.default.url(
            for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true
        )
        // Keep the path used by the existing Listener iOS bundle.
        let path = support.appendingPathComponent("com.listener.music", isDirectory: true).path
        Data(path.utf8).withUnsafeBytes { bytes in
            initializeCore(bytes.baseAddress!, bytes.count)
        }
    }

    static func call<T: Decodable>(_ command: String, _ arguments: [String: any Encodable] = [:]) async throws -> T {
        let request = try JSONEncoder().encode(CoreRequest(command: command, arguments: arguments))
        let response: Data = try await withCheckedThrowingContinuation { continuation in
            let context = Unmanaged.passRetained(CoreResponse(continuation)).toOpaque()
            request.withUnsafeBytes { bytes in
                requestCore(bytes.baseAddress!, bytes.count, context) { context, succeeded, bytes, count in
                    let callback = Unmanaged<CoreResponse>.fromOpaque(context).takeRetainedValue()
                    let data = Data(bytes: bytes, count: count)
                    if succeeded {
                        callback.continuation.resume(returning: data)
                    } else {
                        do {
                            callback.continuation.resume(throwing: try JSONDecoder().decode(CoreError.self, from: data))
                        } catch {
                            callback.continuation.resume(throwing: error)
                        }
                    }
                }
            }
        }
        return try JSONDecoder().decode(T.self, from: response)
    }

    static func perform(_ command: String, _ arguments: [String: any Encodable] = [:]) async throws {
        let _: Bool? = try await call(command, arguments)
    }

    @MainActor static func authorizeQQ() async throws -> Profile? {
        let challenge: QuickLogin = try await call("quickLogin")
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .first { $0.activationState == .foregroundActive }
        guard var presenter = scene?.keyWindow?.rootViewController else {
            throw CoreError(code: "NO_WINDOW", message: "无法打开 QQ 音乐授权。")
        }
        while let presented = presenter.presentedViewController { presenter = presented }
        let callback: String? = try await withCheckedThrowingContinuation { continuation in
            let context = Unmanaged.passRetained(QQResponse(continuation)).toOpaque()
            let controller = Unmanaged.passUnretained(presenter).toOpaque()
            Data(challenge.url.utf8).withUnsafeBytes { bytes in
                presentQQ(controller, bytes.baseAddress!, bytes.count, context) { context, status, bytes, count in
                    let reply = Unmanaged<QQResponse>.fromOpaque(context).takeRetainedValue()
                    switch status {
                    case 0:
                        guard let bytes, let url = String(bytes: Data(bytes: bytes, count: count), encoding: .utf8) else {
                            reply.continuation.resume(throwing: CoreError(code: "QQ_CALLBACK", message: "QQ 音乐的授权地址无效。"))
                            return
                        }
                        reply.continuation.resume(returning: url)
                    case 1: reply.continuation.resume(returning: nil)
                    default:
                        let message = switch status {
                        case 3: "无法打开手机 QQ，请确认已安装。"
                        case 4: "QQ 没有将授权交回 Listener。可以使用二维码登录。"
                        default: "无法加载 QQ 音乐授权页。"
                        }
                        reply.continuation.resume(throwing: CoreError(code: "QQ_LOGIN", message: message))
                    }
                }
            }
        }
        guard let callback else {
            try await perform("cancelLogin")
            return nil
        }
        return try await call("finishQuickLogin", ["callback": callback, "generation": challenge.generation])
    }

    @MainActor static func resume(_ url: URL) {
        Data(url.absoluteString.utf8).withUnsafeBytes { bytes in
            openCoreURL(bytes.baseAddress!, bytes.count)
        }
    }
}

struct Profile: Decodable {
    let uin, nickname, avatar: String
    let offline: Bool
}

struct Track: Codable, Identifiable, Hashable {
    let id: UInt64
    let mid, mediaMid, title, artist, album, cover: String
    let duration: UInt64
    let vip: Bool
    let artists: [Artist]
    let albumMid: String
}

struct Artist: Codable, Identifiable, Hashable {
    let mid, name, image: String
    var id: String { mid }
}

struct Album: Codable, Identifiable, Hashable {
    let mid, name, credit, cover, publishDate, albumType: String
    let artists: [Artist]
    var id: String { mid }
    var releaseYear: String? {
        let prefix = publishDate.prefix(4)
        guard prefix.count == 4, let year = Int(prefix), year >= 1000 else { return nil }
        return String(prefix)
    }
}

struct ArtistDetail: Decodable {
    let artist: Artist
    let description: String
    let totalSongs, totalAlbums: UInt64
}

struct AlbumDetail: Decodable {
    let album: Album
    let description: String
    let tracks: [Track]
}

struct Playlist: Decodable, Identifiable, Hashable {
    let id, name, cover, kind: String
    let count: UInt64
    let creator: String?
    let activityTime: UInt64?
}

struct Library: Decodable { let playlists: [Playlist]; let warnings: [String] }
struct Page<Item: Decodable>: Decodable { let items: [Item]; let nextOffset: UInt64; let hasMore: Bool }
struct PlaylistTrackDate: Decodable { let addedAt: UInt64?; let publishedOn: String? }
struct PlaylistPage: Decodable {
    let tracks: [Track]
    let nextOffset: UInt64
    let hasMore: Bool
    let dates: [String: PlaylistTrackDate]?
}
struct Daily: Decodable { let playlist: Playlist; let tracks: [Track] }
struct Chart: Decodable, Identifiable { let playlist: Playlist; var id: String { playlist.id } }
struct Saved: Decodable {
    let artists: [Artist]
    let albums: [Album]
    let artistSavedAt, albumSavedAt: [String: UInt64]?
}
struct Download: Decodable { let track: Track }
struct Source: Decodable { let url, quality: String; let local: Bool }
struct QqMixPlan: Decodable { let out, input, duration: Double }
struct QuickLogin: Decodable { let url: String; let generation: UInt64 }
struct LoginPoll: Decodable { let status: String; let profile: Profile? }
struct QR: Decodable {
    let id, image: String
    var data: Data? {
        guard let comma = image.firstIndex(of: ",") else { return nil }
        return Data(base64Encoded: String(image[image.index(after: comma)...]))
    }
}

struct Lyrics: Decodable {
    let lines: [LyricLine]
    let warnings: [String]
    let offset: Int64?
    let wordTimed: Bool?
    let interludes: [LyricInterlude]
    let sustainedWords: [Set<Int>]

    private enum CodingKeys: String, CodingKey { case lines, warnings, offset, wordTimed }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        lines = try values.decode([LyricLine].self, forKey: .lines)
        warnings = try values.decode([String].self, forKey: .warnings)
        offset = try values.decodeIfPresent(Int64.self, forKey: .offset)
        wordTimed = try values.decodeIfPresent(Bool.self, forKey: .wordTimed)

        var gaps: [LyricInterlude] = []
        var sungThrough: UInt64 = 0
        for (index, line) in lines.enumerated() {
            // Keep the same seven-second threshold as the desktop lyric renderer.
            if !line.isBg && line.startTime >= sungThrough && line.startTime - sungThrough >= 7000 {
                gaps.append(LyricInterlude(nextLine: index, start: sungThrough, end: line.startTime))
            }
            sungThrough = max(sungThrough, max(line.startTime, line.endTime))
        }
        interludes = gaps
        let hasWordTiming = wordTimed == true
        sustainedWords = lines.map { hasWordTiming ? Self.longNotes(in: $0) : [] }
    }

    func activeIndex(position: Double, userOffset: Int64) -> Int? {
        let time = (position * 1000 + Double(userOffset + (offset ?? 0))).rounded()
        return lines.indices.last { !lines[$0].isBg && Double(lines[$0].startTime) <= time }
    }

    private static func longNotes(in line: LyricLine) -> Set<Int> {
        let words = line.words
        let sung = words.indices.filter { words[$0].word.contains(where: \.isLetter) }
        guard sung.count >= 3 else { return [] }
        let boundaries = sung.dropFirst().map { words[$0].startTime } + [line.endTime]
        let durations = sung.indices.map { index -> Double in
            let word = words[sung[index]]
            let end = min(word.endTime, boundaries[index])
            return max(0, Double(end) - Double(word.startTime))
        }
        var result = Set<Int>()
        for index in sung.indices {
            let word = words[sung[index]]
            let isLast = index == sung.count - 1
            let token = word.word.trimmingCharacters(in: .whitespacesAndNewlines.union(.punctuationCharacters))
            let count = token.count
            guard (1...7).contains(count), token.allSatisfy(\.isLetter),
                  count == 1 || token.unicodeScalars.allSatisfy(\.isASCII) else { continue }
            let duration = durations[index]
            // A sentence-ending hold needs its own word timestamp, close to the actual line end.
            guard duration >= max(isLast ? 1950 : 1750, Double(count) * 335),
                  abs(Double(boundaries[index]) - Double(word.endTime)) <= 260 else { continue }
            let neighbors = (max(0, index - 2)...min(sung.count - 1, index + 2)).compactMap { neighbor -> Double? in
                guard neighbor != index, durations[neighbor] >= 80,
                      neighbor != sung.count - 1 || abs(Double(line.endTime) - Double(words[sung[neighbor]].endTime)) <= 260
                else { return nil }
                return durations[neighbor]
            }.sorted()
            guard neighbors.count >= 2, duration >= neighbors[neighbors.count / 2] * (isLast ? 3.6 : 3.2) else { continue }
            result.insert(sung[index])
        }
        return result
    }
}

struct LyricInterlude {
    let nextLine: Int
    let start, end: UInt64
}

struct LyricLine: Decodable {
    let startTime, endTime: UInt64
    let words: [LyricWord]
    let translatedLyric, romanLyric: String
    let isBg: Bool
    var text: String { words.map(\.word).joined() }
}

struct LyricWord: Decodable {
    let startTime, endTime: UInt64
    let word: String
}

struct Playback: Codable {
    let queue: [Track]
    let currentMid, nextMid: String?
    let position: Double
    let repeatMode: Repeat
    let shuffle: Bool

    enum CodingKeys: String, CodingKey {
        case queue, currentMid, nextMid, position, shuffle
        case repeatMode = "repeat"
    }

    enum Repeat: String, Codable, CaseIterable {
        case off, all, one
        var title: String {
            switch self {
            case .off: "顺序播放"
            case .all: "列表循环"
            case .one: "单曲循环"
            }
        }
    }
}
