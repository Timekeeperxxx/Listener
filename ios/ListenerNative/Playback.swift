import AVFoundation
import MediaPlayer
import Observation
import SwiftUI
import UIKit

@MainActor @Observable
final class Player {
    @ObservationIgnored private var audio = AVPlayer()
    @ObservationIgnored private var timeObserver: Any?
    @ObservationIgnored private var endedObserver: NSObjectProtocol?
    @ObservationIgnored private var itemObserver: NSKeyValueObservation?
    @ObservationIgnored private var rateObserver: NSKeyValueObservation?
    @ObservationIgnored private var loadTask: Task<Void, Never>?
    @ObservationIgnored private var lyricsTask: Task<Void, Never>?
    @ObservationIgnored private var saveTask: Task<Void, Never>?
    @ObservationIgnored private var lastSave = Date.distantPast
    @ObservationIgnored private var revision = 0
    @ObservationIgnored private var collectionIntent = 0
    @ObservationIgnored private var pendingSeek: UUID?
    @ObservationIgnored private var artworkTask: Task<Void, Never>?
    @ObservationIgnored private var artworkURL: String?
    @ObservationIgnored private var artwork: MPMediaItemArtwork?
    @ObservationIgnored private var mixEnabled = false
    @ObservationIgnored private var mixGeneration = 0
    @ObservationIgnored private var mixPair: [String] = []
    @ObservationIgnored private var mixPlanTask: Task<Void, Never>?
    @ObservationIgnored private var mixSourceTask: Task<Void, Never>?
    @ObservationIgnored private var preparedMix: PreparedMix?
    @ObservationIgnored private var fading: ActiveMix?
    @ObservationIgnored private var fadeObserver: Any?

    private(set) var owner: String?
    private(set) var current: Track?
    private(set) var queue: [Track] = []
    @ObservationIgnored private(set) var queueRevision = 0
    private(set) var nextMid: String?
    private(set) var playing = false
    private(set) var loading = false
    private(set) var position: Double = 0
    private(set) var seekRevision = 0
    private(set) var quality = ""
    private(set) var lyrics: Lyrics?
    private(set) var lyricError: String?
    private(set) var lyricsLoading = false
    private(set) var mixNotice: String?
    private(set) var coverArtwork: UIImage?
    private(set) var coverTone = CoverTone.neutral
    var error: String?
    var shuffle = false
    var repeatMode = Playback.Repeat.off

    var duration: Double {
        guard let current else { return 0 }
        let resolved = audio.currentItem?.duration.seconds ?? 0
        return resolved.isFinite && resolved > 0 ? resolved : Double(current.duration)
    }

    var lyricPosition: Double {
        let time = audio.currentTime().seconds
        return pendingSeek == nil && audio.currentItem != nil && time.isFinite ? time : position
    }

    init() {
        bindAudioClock()
        endedObserver = NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime, object: nil, queue: .main) { [weak self] note in
            Task { @MainActor in
                guard let self, let item = note.object as? AVPlayerItem else { return }
                if item === self.audio.currentItem { self.advance(automatic: true) }
                else if item === self.fading?.outgoing.currentItem { self.finishMix() }
            }
        }
        let commands = MPRemoteCommandCenter.shared()
        commands.playCommand.addTarget { [weak self] _ in
            Task { @MainActor in self?.resume() }
            return .success
        }
        commands.pauseCommand.addTarget { [weak self] _ in
            Task { @MainActor in self?.pause() }
            return .success
        }
        commands.togglePlayPauseCommand.addTarget { [weak self] _ in
            Task { @MainActor in self?.toggle() }
            return .success
        }
        commands.nextTrackCommand.addTarget { [weak self] _ in
            Task { @MainActor in self?.advance() }
            return .success
        }
        commands.previousTrackCommand.addTarget { [weak self] _ in
            Task { @MainActor in self?.previous() }
            return .success
        }
        commands.changePlaybackPositionCommand.addTarget { [weak self] event in
            guard let time = (event as? MPChangePlaybackPositionCommandEvent)?.positionTime else { return .commandFailed }
            Task { @MainActor in self?.seek(time) }
            return .success
        }
    }

    private func bindAudioClock() {
        let deck = audio
        timeObserver = deck.addPeriodicTimeObserver(forInterval: CMTime(seconds: 0.2, preferredTimescale: 600), queue: .main) { [weak self, weak deck] _ in
            Task { @MainActor in
                guard let self, self.audio === deck, self.pendingSeek == nil, self.audio.currentItem != nil else { return }
                self.position = self.lyricPosition
                if self.playing {
                    self.updateMix(at: self.position)
                    if Date().timeIntervalSince(self.lastSave) >= 15 { self.persist() }
                }
            }
        }
        rateObserver = deck.observe(\.timeControlStatus, options: [.new]) { [weak self, weak deck] _, _ in
            Task { @MainActor in
                guard let self, let deck, self.audio === deck else { return }
                self.playing = deck.timeControlStatus == .playing
                self.publishNowPlaying()
            }
        }
    }

    func connect(_ newOwner: String?) async {
        guard owner != newOwner else { return }
        revision += 1
        collectionIntent += 1
        cancelUpcomingMix()
        finishMix()
        mixNotice = nil
        pendingSeek = nil
        loadTask?.cancel()
        lyricsTask?.cancel()
        audio.pause()
        audio.replaceCurrentItem(with: nil)
        current = nil
        queue = []
        queueRevision += 1
        nextMid = nil
        position = 0
        lyrics = nil
        lyricError = nil
        lyricsLoading = false
        playing = false
        loading = false
        quality = ""
        error = nil
        owner = newOwner
        publishNowPlaying()
        guard let newOwner else { return }
        do {
            let saved: Playback? = try await Core.call("playback", ["owner": newOwner])
            guard owner == newOwner, current == nil, let saved else { return }
            queue = saved.queue
            current = queue.first { $0.mid == saved.currentMid }
            nextMid = saved.nextMid
            position = saved.position
            repeatMode = saved.repeatMode
            shuffle = saved.shuffle
            if let current { fetchLyrics(for: current) }
            publishNowPlaying()
        } catch {
            if owner == newOwner { self.error = error.localizedDescription }
        }
    }

    func playCollection(_ tracks: [Track], shuffled: Bool) {
        guard owner != nil else { return }
        var seen = Set<String>()
        let tracks = tracks.filter { seen.insert($0.mid).inserted }
        guard let first = shuffled ? tracks.randomElement() : tracks.first else { return }
        shuffle = shuffled
        play(first, in: tracks)
    }

    func reserveCollectionPlayback() -> Int {
        collectionIntent += 1
        return collectionIntent
    }

    func collectionPlaybackIsCurrent(_ token: Int) -> Bool { token == collectionIntent }

    func play(_ track: Track, in tracks: [Track]? = nil, from time: Double = 0) {
        guard owner != nil else { return }
        collectionIntent += 1
        cancelUpcomingMix()
        finishMix()
        if let tracks {
            queueRevision += 1
            queue = shuffle ? [track] + tracks.filter { $0.mid != track.mid }.shuffled() : tracks
            nextMid = nil
        } else if !queue.contains(where: { $0.mid == track.mid }) {
            queue.append(track)
        }
        if nextMid == track.mid { nextMid = nil }
        let trackChanged = current?.mid != track.mid
        revision += 1
        pendingSeek = nil
        let request = revision
        loadTask?.cancel()
        audio.pause()
        audio.replaceCurrentItem(with: nil)
        current = track
        position = time
        quality = ""
        loading = true
        error = nil
        if trackChanged || lyrics == nil { fetchLyrics(for: track) }
        publishNowPlaying()
        loadTask = Task { [weak self] in
            do {
                let source: Source = try await Core.call("source", ["track": track])
                guard let self, request == self.revision, !Task.isCancelled else { return }
                guard let url = source.local ? URL(fileURLWithPath: source.url) : URL(string: source.url) else {
                    throw CoreError(code: "SOURCE_URL", message: "播放地址无效。")
                }
                let session = AVAudioSession.sharedInstance()
                do {
                    if session.category != .playback { try session.setCategory(.playback) }
                } catch {
                    throw CoreError(code: "AUDIO_SESSION", message: "无法启用 iPhone 音频会话。")
                }
                let item = AVPlayerItem(url: url)
                self.quality = source.quality
                self.itemObserver = item.observe(\.status, options: [.initial, .new]) { [weak self] item, _ in
                    Task { @MainActor in
                        guard let self, request == self.revision else { return }
                        if item.status == .failed {
                            self.loading = false
                            self.error = "音频无法播放，请重新尝试。"
                        } else if item.status == .readyToPlay && self.loading {
                            self.loading = false
                            self.publishNowPlaying()
                            self.persist()
                            do { try await Core.perform("record", ["track": track]) }
                            catch { if request == self.revision { self.error = error.localizedDescription } }
                        }
                    }
                }
                let requestedPosition = self.position
                self.audio.replaceCurrentItem(with: item)
                if requestedPosition > 0 { self.movePlayhead(to: requestedPosition) }
                self.audio.play()
            } catch {
                guard let self, request == self.revision, !Task.isCancelled else { return }
                self.error = error.localizedDescription
                self.loading = false
            }
        }
        persist()
    }

    func fetchLyrics(for track: Track) {
        lyricsTask?.cancel()
        lyrics = nil
        lyricError = nil
        lyricsLoading = true
        lyricsTask = Task {
            do {
                let value: Lyrics = try await Core.call("lyrics", ["track": track])
                guard !Task.isCancelled, current?.mid == track.mid else { return }
                lyrics = value
            } catch {
                guard !Task.isCancelled, current?.mid == track.mid else { return }
                lyricError = error.localizedDescription
            }
            lyricsLoading = false
        }
    }

    func toggle() {
        if playing || loading { pause() }
        else { resume() }
    }

    func resume() {
        guard let current else { return }
        if let item = audio.currentItem, item.status != .failed {
            if duration > 0 && position >= duration - 0.1 { seek(0) }
            audio.play()
        } else {
            play(current, from: position)
        }
    }

    func pause() {
        cancelUpcomingMix()
        finishMix()
        if loading {
            revision += 1
            pendingSeek = nil
            loadTask?.cancel()
            audio.replaceCurrentItem(with: nil)
            loading = false
        }
        audio.pause()
        persist()
    }

    func seek(_ time: Double) {
        seekRevision += 1
        movePlayhead(to: time)
    }

    private func movePlayhead(to time: Double) {
        if preparedMix?.starting == true { cancelUpcomingMix() }
        finishMix()
        position = min(max(0, time), max(0, duration))
        if audio.currentItem != nil {
            let request = UUID()
            pendingSeek = request
            audio.seek(to: CMTime(seconds: position, preferredTimescale: 1000),
                       toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
                Task { @MainActor in
                    if self?.pendingSeek == request { self?.pendingSeek = nil }
                }
            }
        }
        publishNowPlaying()
        persist()
    }

    func advance(automatic: Bool = false) {
        if automatic, repeatMode == .one { seek(0); resume(); return }
        if let next = nextTrack { play(next) }
        else if automatic { pause() }
    }

    private var nextTrack: Track? {
        if let next = queue.first(where: { $0.mid == nextMid }) { return next }
        guard let index = queue.firstIndex(where: { $0.mid == current?.mid }) else { return nil }
        if index + 1 < queue.count { return queue[index + 1] }
        return repeatMode == .all ? queue.first : nil
    }

    func configureAutoMix(_ enabled: Bool) {
        guard enabled != mixEnabled else { return }
        mixEnabled = enabled
        if !enabled {
            cancelUpcomingMix()
            finishMix()
            mixNotice = nil
        } else if playing {
            updateMix(at: lyricPosition)
        }
    }

    private func updateMix(at time: Double) {
        guard mixEnabled, fading == nil, !loading, repeatMode != .one,
              let from = current, let next = nextTrack, from.mid != next.mid else {
            if fading == nil && !mixPair.isEmpty { cancelUpcomingMix() }
            return
        }
        let index = queue.firstIndex { $0.mid == from.mid }
        let nextInOrder = index.flatMap { queue.indices.contains($0 + 1) ? queue[$0 + 1].mid : nil }
        if !shuffle && nextMid == nil && !from.albumMid.isEmpty && from.albumMid == next.albumMid && nextInOrder == next.mid {
            if !mixPair.isEmpty { cancelUpcomingMix() }
            return
        }
        let pair = [from.mid, next.mid]
        if mixPair != pair {
            cancelUpcomingMix()
            mixPair = pair
            mixNotice = nil
            let generation = mixGeneration
            mixPlanTask = Task { [weak self] in
                do {
                    let plan: QqMixPlan = try await Core.call("mixPlan", ["from": from, "to": next])
                    guard let self, generation == self.mixGeneration, !Task.isCancelled else { return }
                    guard plan.out.isFinite, plan.input.isFinite, plan.duration.isFinite, plan.out >= 0,
                          plan.input >= 0, plan.duration > 0.25, plan.out + plan.duration <= self.duration + 0.25,
                          plan.input + plan.duration <= Double(next.duration) + 1 else {
                        self.rejectMix("QQ 返回的衔接点超出音频时长。")
                        return
                    }
                    self.preparedMix = PreparedMix(track: next, plan: plan)
                    if self.playing { self.updateMix(at: self.lyricPosition) }
                } catch {
                    guard let self, generation == self.mixGeneration, !Task.isCancelled else { return }
                    self.rejectMix("这两首歌无法使用 QQ 自动衔接：\(error.localizedDescription)")
                }
            }
        }
        guard let preparedMix else { return }
        let plan = preparedMix.plan
        if time > plan.out + min(0.9, plan.duration * 0.35) {
            rejectMix("已错过 QQ 衔接点，这次按原曲结束位置切换。")
        } else if preparedMix.ready && !preparedMix.starting && time >= plan.out {
            startMix(preparedMix, elapsed: time - plan.out)
        } else if preparedMix.deck == nil && mixSourceTask == nil && time >= plan.out - 25 {
            prepareMixAudio(preparedMix)
        }
    }

    private func prepareMixAudio(_ candidate: PreparedMix) {
        mixSourceTask = Task { [weak self, weak candidate] in
            guard let self, let candidate else { return }
            do {
                let source: Source = try await Core.call("source", ["track": candidate.track])
                guard self.preparedMix === candidate, !Task.isCancelled else { return }
                guard let url = source.local ? URL(fileURLWithPath: source.url) : URL(string: source.url) else {
                    self.rejectMix("下一首的播放地址无效，无法自动衔接。")
                    return
                }
                let item = AVPlayerItem(url: url)
                let deck = AVPlayer(playerItem: item)
                deck.volume = 0
                candidate.deck = deck
                candidate.source = source
                candidate.itemObservation = item.observe(\.status, options: [.initial, .new]) { [weak self, weak candidate] item, _ in
                    Task { @MainActor in
                        guard let self, let candidate, self.preparedMix === candidate else { return }
                        if item.status == .failed {
                            self.rejectMix("下一首未能预加载，改为正常切换。")
                        } else if item.status == .readyToPlay && !candidate.cueRequested {
                            self.cueMixAudio(candidate)
                        }
                    }
                }
            } catch {
                guard self.preparedMix === candidate, !Task.isCancelled else { return }
                self.rejectMix("自动衔接的下一首未能加载：\(error.localizedDescription)")
            }
        }
    }

    private func cueMixAudio(_ candidate: PreparedMix) {
        guard let deck = candidate.deck else { return }
        candidate.cueRequested = true
        let duration = deck.currentItem?.duration.seconds ?? 0
        if duration.isFinite && duration > 0 && candidate.plan.input + candidate.plan.duration > duration + 0.25 {
            rejectMix("下一首剩余音频不足，无法自动衔接。")
            return
        }
        deck.seek(to: CMTime(seconds: candidate.plan.input, preferredTimescale: 1000),
                  toleranceBefore: .zero, toleranceAfter: .zero) { [weak self, weak candidate, weak deck] finished in
            Task { @MainActor in
                guard let self, let candidate, let deck, self.preparedMix === candidate else { return }
                guard finished else {
                    self.rejectMix("下一首未能定位到衔接点，改为正常切换。")
                    return
                }
                guard deck.status == .readyToPlay else {
                    self.rejectMix("下一首尚未就绪，改为正常切换。")
                    return
                }
                deck.preroll(atRate: 1) { [weak self, weak candidate] buffered in
                    Task { @MainActor in
                        guard let self, let candidate, self.preparedMix === candidate else { return }
                        if buffered { candidate.ready = true }
                        else { self.rejectMix("下一首预缓冲未完成，改为正常切换。") }
                    }
                }
            }
        }
    }

    private func startMix(_ candidate: PreparedMix, elapsed: Double) {
        guard let deck = candidate.deck else { return }
        candidate.starting = true
        candidate.rateObservation = deck.observe(\.timeControlStatus, options: [.new]) { [weak self, weak candidate] deck, _ in
            Task { @MainActor in
                guard let self, let candidate, self.preparedMix === candidate, deck.timeControlStatus == .playing else { return }
                self.acceptMix(candidate)
            }
        }
        if elapsed <= 0.065 {
            deck.play()
        } else {
            deck.seek(to: CMTime(seconds: candidate.plan.input + elapsed, preferredTimescale: 1000),
                      toleranceBefore: .zero, toleranceAfter: .zero) { [weak self, weak candidate, weak deck] finished in
                Task { @MainActor in
                    guard let self, let candidate, let deck, self.preparedMix === candidate else { return }
                    if finished { deck.play() }
                    else { self.rejectMix("无法对齐 QQ 衔接点，改为正常切换。") }
                }
            }
        }
    }

    private func acceptMix(_ candidate: PreparedMix) {
        guard let incoming = candidate.deck, let source = candidate.source, let item = incoming.currentItem else { return }
        let elapsed = lyricPosition - candidate.plan.out
        guard mixEnabled, playing, repeatMode != .one, nextTrack?.mid == candidate.track.mid,
              elapsed >= 0, elapsed < min(0.9, candidate.plan.duration * 0.35) else {
            rejectMix("已错过 QQ 衔接点，这次按原曲结束位置切换。")
            return
        }
        let outgoing = audio
        if let timeObserver { outgoing.removeTimeObserver(timeObserver) }
        candidate.itemObservation = nil
        candidate.rateObservation = nil
        preparedMix = nil
        mixPair = []
        mixPlanTask = nil
        mixSourceTask = nil
        revision += 1
        let request = revision
        pendingSeek = nil
        audio = incoming
        let track = candidate.track
        current = track
        if nextMid == track.mid { nextMid = nil }
        let incomingTime = incoming.currentTime().seconds
        position = incomingTime.isFinite ? incomingTime : candidate.plan.input + elapsed
        quality = source.quality
        loading = false
        error = nil
        itemObserver = item.observe(\.status, options: [.new]) { [weak self] item, _ in
            Task { @MainActor in
                guard let self, request == self.revision, item.status == .failed else { return }
                self.finishMix()
                self.error = "音频无法播放，请重新尝试。"
            }
        }
        bindAudioClock()
        fading = ActiveMix(outgoing: outgoing, start: position, elapsed: elapsed, duration: candidate.plan.duration)
        fadeObserver = incoming.addPeriodicTimeObserver(forInterval: CMTime(seconds: 0.025, preferredTimescale: 1000),
                                                        queue: .main) { [weak self] _ in
            Task { @MainActor in self?.stepMixFade() }
        }
        stepMixFade()
        mixNotice = "已使用 QQ 自动衔接。"
        fetchLyrics(for: track)
        publishNowPlaying()
        persist()
        Task {
            do { try await Core.perform("record", ["track": track]) }
            catch { if request == revision { self.error = error.localizedDescription } }
        }
    }

    private func stepMixFade() {
        guard let fading else { return }
        let seconds = audio.currentTime().seconds
        guard seconds.isFinite else { return }
        let fraction = min(1, (fading.elapsed + max(0, seconds - fading.start)) / fading.duration)
        if fraction >= 1 { finishMix(); return }
        fading.outgoing.volume = mixVolume(QqGain.outgoing, at: fraction)
        audio.volume = mixVolume(QqGain.incoming, at: fraction)
    }

    private func finishMix() {
        guard let fading else { return }
        if let fadeObserver { audio.removeTimeObserver(fadeObserver) }
        fadeObserver = nil
        self.fading = nil
        fading.outgoing.pause()
        fading.outgoing.replaceCurrentItem(with: nil)
        audio.volume = 1
    }

    private func rejectMix(_ message: String) {
        mixNotice = message
        cancelUpcomingMix(resetPair: false)
    }

    private func cancelUpcomingMix(resetPair: Bool = true) {
        mixGeneration += 1
        if resetPair { mixPair = [] }
        mixPlanTask?.cancel()
        mixSourceTask?.cancel()
        mixPlanTask = nil
        mixSourceTask = nil
        if let candidate = preparedMix {
            preparedMix = nil
            candidate.deck?.cancelPendingPrerolls()
            candidate.deck?.pause()
            candidate.deck?.replaceCurrentItem(with: nil)
        }
    }

    func previous() {
        if position > 3 { seek(0); return }
        guard let index = queue.firstIndex(where: { $0.mid == current?.mid }), index > 0 else {
            seek(0)
            return
        }
        play(queue[index - 1])
    }

    func add(_ track: Track, next: Bool) {
        guard track.mid != current?.mid else { return }
        queue.removeAll { $0.mid == track.mid }
        if next, let index = queue.firstIndex(where: { $0.mid == current?.mid }) {
            queue.insert(track, at: index + 1)
            nextMid = track.mid
        } else {
            queue.append(track)
        }
        if current == nil {
            current = track
            fetchLyrics(for: track)
            publishNowPlaying()
        }
        persist()
    }

    func appendPlaylistPage(_ tracks: [Track], revision: Int) -> Bool {
        guard queueRevision == revision else { return false }
        var known = Set(queue.map(\.mid))
        let new = tracks.filter { known.insert($0.mid).inserted }
        queue.append(contentsOf: shuffle ? new.shuffled() : new)
        persist()
        return true
    }

    func move(from offsets: IndexSet, to index: Int) {
        queue.move(fromOffsets: offsets, toOffset: index)
        nextMid = nil
        persist()
    }

    func remove(_ track: Track) {
        guard current?.mid != track.mid else { return }
        queue.removeAll { $0.mid == track.mid }
        if nextMid == track.mid { nextMid = nil }
        persist()
    }

    func cycleRepeat() {
        repeatMode = switch repeatMode { case .off: .all; case .all: .one; case .one: .off }
        persist()
    }

    func toggleShuffle() {
        shuffle.toggle()
        if shuffle, let index = queue.firstIndex(where: { $0.mid == current?.mid }) {
            queue = Array(queue.prefix(index + 1)) + queue.dropFirst(index + 1).shuffled()
            nextMid = nil
        }
        persist()
    }

    func persist() {
        guard let owner, current != nil else { return }
        lastSave = Date()
        let playback = Playback(queue: queue, currentMid: current?.mid, nextMid: nextMid,
                                position: position, repeatMode: repeatMode, shuffle: shuffle)
        let previous = saveTask
        saveTask = Task {
            await previous?.value
            guard self.owner == owner else { return }
            do { try await Core.perform("savePlayback", ["owner": owner, "playback": playback]) }
            catch { if self.owner == owner { self.error = error.localizedDescription } }
        }
    }

    func flush() async {
        persist()
        await saveTask?.value
    }

    private func publishNowPlaying() {
        updateNowPlayingArtwork(for: current?.cover)
        guard let current else {
            MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
            return
        }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: current.title, MPMediaItemPropertyArtist: current.artist,
            MPMediaItemPropertyAlbumTitle: current.album,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.audio.rawValue,
            MPNowPlayingInfoPropertyExternalContentIdentifier: current.mid,
            MPMediaItemPropertyPlaybackDuration: duration, MPNowPlayingInfoPropertyElapsedPlaybackTime: position,
            MPNowPlayingInfoPropertyPlaybackRate: playing ? 1 : 0,
        ]
        if let artwork { info[MPMediaItemPropertyArtwork] = artwork }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = info
    }

    private func updateNowPlayingArtwork(for cover: String?) {
        guard cover != artworkURL else { return }
        artworkTask?.cancel()
        artworkURL = cover
        artwork = nil
        coverArtwork = nil
        coverTone = .neutral
        guard let cover, !cover.isEmpty, let url = URL(string: cover) else { return }
        artworkTask = Task { [weak self] in
            // Album art is optional on the system media surface; audio and controls stay available.
            guard let (data, response) = try? await URLSession.shared.data(from: url),
                  (response as? HTTPURLResponse)?.statusCode == 200,
                  let image = UIImage(data: data), !Task.isCancelled,
                  let self, artworkURL == cover else { return }
            coverArtwork = image
            coverTone = CoverTone.sample(image)
            artwork = MPMediaItemArtwork(boundsSize: image.size) { _ in image }
            publishNowPlaying()
        }
    }
}

@MainActor
private final class PreparedMix {
    let track: Track
    let plan: QqMixPlan
    var deck: AVPlayer?
    var source: Source?
    var itemObservation: NSKeyValueObservation?
    var rateObservation: NSKeyValueObservation?
    var cueRequested = false
    var ready = false
    var starting = false

    init(track: Track, plan: QqMixPlan) {
        self.track = track
        self.plan = plan
    }
}

private struct ActiveMix {
    let outgoing: AVPlayer
    let start, elapsed, duration: Double
}

private enum QqGain {
    // Same QQ amfilter2 gain points used by the macOS implementation.
    static let outgoing: [(Double, Double)] = [(0, 0), (0.5, 0), (0.75, -2), (0.85, -6), (0.92, -15), (1, -35)]
    static let incoming: [(Double, Double)] = [(0.35, -30), (0.5, -15), (0.7, -7), (0.85, -3), (0.92, 0)]
}

private func mixVolume(_ points: [(Double, Double)], at fraction: Double) -> Float {
    var previous = points[0]
    var gain = previous.1
    if fraction > previous.0 {
        for point in points.dropFirst() {
            if fraction <= point.0 {
                gain = previous.1 + (point.1 - previous.1) * (fraction - previous.0) / (point.0 - previous.0)
                break
            }
            gain = point.1
            previous = point
        }
    }
    return Float(pow(10, gain / 20))
}
