import SwiftUI
import Observation

enum Destination: Hashable {
    case playlist(Playlist)
    case artist(Artist)
    case album(String)
    case albums
    case artists
    case recent
    case downloads
    case savedAlbums
    case savedArtists
    case recommendedSongs([Track])
}

extension EnvironmentValues {
    @Entry var catalogRouter: CatalogRouter?
}

@MainActor @Observable
final class CatalogRouter {
    var path: [Destination] = []

    func open(_ destination: Destination) { path.append(destination) }
}

struct CatalogNavigation<Content: View>: View {
    @State private var router = CatalogRouter()
    @ViewBuilder var content: () -> Content

    var body: some View {
        @Bindable var router = router
        NavigationStack(path: $router.path) { content().catalogDestinations() }
            .environment(\.catalogRouter, router)
    }
}

extension View {
    func catalogDestinations() -> some View {
        navigationDestination(for: Destination.self) { route in
            switch route {
            case let .playlist(playlist): PlaylistDetail(playlist: playlist)
            case let .artist(artist): ArtistPage(artist: artist)
            case let .album(mid): AlbumPage(mid: mid)
            case .albums: DirectoryPage(artists: false)
            case .artists: DirectoryPage(artists: true)
            case .recent: LocalTracks(title: "最近播放", downloaded: false)
            case .downloads: LocalTracks(title: "下载与离线", downloaded: true)
            case .savedAlbums: SavedPage(artists: false)
            case .savedArtists: SavedPage(artists: true)
            case let .recommendedSongs(tracks): RecommendedSongs(initial: tracks)
            }
        }
    }
}

struct HomePage: View {
    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player
    private var feed: HomeFeed { model.home }
    private let previewCount = 4

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 26) {
                if model.profile?.offline == true {
                    Label("当前离线", systemImage: "wifi.slash").foregroundStyle(.secondary)
                    NavigationLink("打开下载与离线", value: Destination.downloads)
                } else {
                    dailySection
                    section("为你推荐", value: feed.radio) { tracks in
                        ForEach(tracks.prefix(previewCount)) { track in TrackRow(track: track, tracks: tracks) }
                        if tracks.isEmpty { empty("暂无推荐歌曲") }
                        else {
                            NavigationLink("查看全部", value: Destination.recommendedSongs(tracks))
                                .font(.subheadline.weight(.semibold))
                                .frame(minHeight: 44)
                        }
                    } placeholder: {
                        ForEach(0..<previewCount, id: \.self) { _ in
                            HomeRowPlaceholder(coverSide: 34, spacing: 9)
                                .frame(minHeight: 44)
                        }
                        HomeGlassPlaceholder(radius: 10).frame(width: 66, height: 26)
                            .frame(minHeight: 44)
                    }
                    section("推荐歌单", value: feed.playlists) { values in
                        if values.isEmpty { empty("暂无推荐歌单") }
                        recordShelf(values) { playlist, side in
                            NavigationLink(value: Destination.playlist(playlist)) {
                                RecordTile(title: playlist.name, subtitle: nil, cover: playlist.cover, coverSide: side)
                            }
                            .buttonStyle(.plain)
                        }
                    } placeholder: {
                        recordShelf {
                            ForEach(0..<3, id: \.self) { _ in
                                VStack(alignment: .leading, spacing: 5) {
                                    HomeGlassPlaceholder(radius: 12).frame(width: recordShelfSide, height: recordShelfSide)
                                    Text(" ").font(.subheadline.weight(.semibold))
                                        .lineLimit(2, reservesSpace: true)
                                        .overlay(alignment: .topLeading) {
                                            VStack(alignment: .leading, spacing: 7) {
                                                HomeGlassPlaceholder(radius: 4).frame(width: 112, height: 8)
                                                HomeGlassPlaceholder(radius: 4).frame(width: 72, height: 7)
                                            }
                                            .padding(.top, 5)
                                        }
                                }
                            }
                        }
                    }
                    section("排行榜", value: feed.charts) { values in
                        if values.isEmpty { empty("暂无排行榜") }
                        ForEach(values) { chart in PlaylistLink(playlist: chart.playlist) }
                    } placeholder: {
                        ForEach(0..<3, id: \.self) { _ in HomeRowPlaceholder(coverSide: 48, spacing: 12) }
                    }
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
        }
        .navigationTitle("推荐")
        .toolbar { if !model.checkingSession { AccountToolbar() } }
        .task(id: model.checkingSession) { await load(refresh: false) }
        .refreshable { await load(refresh: true) }
    }

    @ViewBuilder private var dailySection: some View {
        switch feed.daily {
        case let .success(value):
            NavigationLink(value: Destination.playlist(value.playlist)) {
                dailyCard {
                    Cover(url: value.tracks.first?.cover ?? value.playlist.cover, radius: 16)
                }
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("daily-card")
            .overlay(alignment: .bottomLeading) {
                if !value.tracks.isEmpty {
                    HStack(spacing: 8) {
                        Button { player.playCollection(value.tracks, shuffled: false) } label: {
                            Label("播放", systemImage: "play.fill")
                        }
                        .buttonStyle(.glassProminent)
                        Button { player.playCollection(value.tracks, shuffled: true) } label: {
                            Image(systemName: "shuffle").frame(minWidth: 28)
                        }
                        .buttonStyle(.glass)
                        .accessibilityLabel("随机播放每日 30 首")
                    }
                    .padding(20)
                }
            }
        case let .failure(error):
            VStack(alignment: .leading, spacing: 10) {
                Text("每日 30 首").font(.title2.bold())
                empty(error.localizedDescription)
                Button("重试") { Task { await load(refresh: true) } }
            }
        case nil:
            dailyCard { HomeGlassPlaceholder(radius: 16) }
                .overlay(alignment: .bottomLeading) {
                    HStack(spacing: 8) {
                        HomeGlassPlaceholder(radius: 18).frame(width: 78, height: 36)
                        HomeGlassPlaceholder(radius: 18).frame(width: 48, height: 36)
                    }
                    .padding(20)
                }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("正在加载每日 30 首")
        }
    }

    private func dailyCard<Artwork: View>(@ViewBuilder artwork: () -> Artwork) -> some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 20) {
                dailyTitle
                artwork().frame(width: 106, height: 106)
            }
            dailyTitle
        }
        .padding(20)
        .frame(maxWidth: .infinity, minHeight: 146, alignment: .leading)
        .contentShape(.rect(cornerRadius: 24))
        .background(.tint.opacity(0.10), in: .rect(cornerRadius: 24))
    }

    private var dailyTitle: some View {
        Text("每日 30 首").font(.title2.bold()).foregroundStyle(.primary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.bottom, 52)
    }

    private func section<Value, Content: View, Placeholder: View>(
        _ title: String, value: Result<Value, Error>?,
        @ViewBuilder content: (Value) -> Content,
        @ViewBuilder placeholder: () -> Placeholder
    ) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.title3.bold())
                .accessibilityAddTraits(.isHeader)
            switch value {
            case let .success(value): content(value)
            case let .failure(error): empty(error.localizedDescription)
            case nil:
                VStack(alignment: .leading, spacing: 8, content: placeholder)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("正在加载\(title)")
            }
        }
    }

    private func empty(_ message: String) -> some View {
        Text(message).font(.subheadline).foregroundStyle(.secondary)
    }

    private func load(refresh: Bool) async {
        guard !model.checkingSession, model.profile?.offline == false else { return }
        await feed.load(refresh: refresh)
    }
}

private struct HomeGlassPlaceholder: View {
    let radius: CGFloat

    var body: some View {
        Color.clear
            .glassEffect(.regular.tint(.accentColor.opacity(0.10)), in: .rect(cornerRadius: radius))
            .accessibilityHidden(true)
            .allowsHitTesting(false)
    }
}

private struct HomeRowPlaceholder: View {
    let coverSide: CGFloat
    let spacing: CGFloat

    var body: some View {
        HStack(spacing: spacing) {
            HomeGlassPlaceholder(radius: 7).frame(width: coverSide, height: coverSide)
            VStack(alignment: .leading, spacing: 6) {
                HomeGlassPlaceholder(radius: 4).frame(width: 118, height: 10)
                HomeGlassPlaceholder(radius: 4).frame(width: 78, height: 8)
            }
            Spacer(minLength: 0)
        }
    }
}

@MainActor @Observable
final class HomeFeed {
    private(set) var daily: Result<Daily, Error>?
    private(set) var radio: Result<[Track], Error>?
    private(set) var playlists: Result<[Playlist], Error>?
    private(set) var charts: Result<[Chart], Error>?
    private(set) var moreRadio: [Track]?
    @ObservationIgnored private var loading: Task<Void, Never>?
    @ObservationIgnored private var revision = 0
    @ObservationIgnored private var dailyDay: Date?

    func load(refresh: Bool) async {
        if let loading { await loading.value; return }
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(secondsFromGMT: 8 * 3600)!
        let day = calendar.startOfDay(for: .now)
        let dailyNeeded = refresh || daily == nil || day != dailyDay
        let radioNeeded = refresh || radio == nil
        let playlistsNeeded = refresh || playlists == nil
        let chartsNeeded = refresh || charts == nil
        guard dailyNeeded || radioNeeded || playlistsNeeded || chartsNeeded else { return }
        if refresh { revision += 1; moreRadio = nil }
        let task = Task {
            async let d: Void = fetchDaily(refresh, needed: dailyNeeded)
            async let r: Void = fetchRadio(radioNeeded)
            async let p: Void = fetchPlaylists(playlistsNeeded)
            async let c: Void = fetchCharts(chartsNeeded)
            _ = await (d, r, p, c)
            if !Task.isCancelled && dailyNeeded { dailyDay = day }
        }
        loading = task
        await task.value
        loading = nil
    }

    func cancel() { loading?.cancel() }

    func loadMoreRadio() async throws -> [Track] {
        if let moreRadio { return moreRadio }
        let request = revision
        let value: [Track] = try await Core.call("radioMore")
        try Task.checkCancellation()
        guard request == revision else { throw CancellationError() }
        moreRadio = value
        return value
    }

    private func fetchDaily(_ refresh: Bool, needed: Bool) async {
        guard needed else { return }
        do { let value: Daily = try await Core.call("daily", ["refresh": refresh]); if !Task.isCancelled { daily = .success(value) } }
        catch { if !Task.isCancelled { daily = .failure(error) } }
    }

    private func fetchRadio(_ needed: Bool) async {
        guard needed else { return }
        do { let value: [Track] = try await Core.call("radio", ["limit": 30]); if !Task.isCancelled { radio = .success(value) } }
        catch { if !Task.isCancelled { radio = .failure(error) } }
    }

    private func fetchPlaylists(_ needed: Bool) async {
        guard needed else { return }
        do { let value: [Playlist] = try await Core.call("recommendations"); if !Task.isCancelled { playlists = .success(value) } }
        catch { if !Task.isCancelled { playlists = .failure(error) } }
    }

    private func fetchCharts(_ needed: Bool) async {
        guard needed else { return }
        do { let value: [Chart] = try await Core.call("charts"); if !Task.isCancelled { charts = .success(value) } }
        catch { if !Task.isCancelled { charts = .failure(error) } }
    }
}

private struct RecommendedSongs: View {
    let initial: [Track]
    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player
    @State private var more: [Track]?
    @State private var error: String?

    private var tracks: [Track] {
        var seen = Set<String>()
        return (initial + (more ?? [])).filter { seen.insert($0.mid).inserted }
    }

    var body: some View {
        List {
            let tracks = tracks
            if !tracks.isEmpty {
                CollectionPlaybackButtons(tracks: tracks, player: player)
                    .disabled(more == nil && initial.count < 30 && error == nil)
            }
            ForEach(tracks) { TrackRow(track: $0, tracks: tracks) }
            if let error { LoadError(message: error) { Task { await load() } } }
            else if more == nil && initial.count < 30 { ProgressView("正在加载更多推荐") }
        }
        .listStyle(.plain)
        .navigationTitle("为你推荐")
        .task { if more == nil && initial.count < 30 { await load() } }
    }

    private func load() async {
        error = nil
        do { more = try await model.home.loadMoreRadio() }
        catch is CancellationError {}
        catch { if !Task.isCancelled { self.error = error.localizedDescription } }
    }
}

struct DiscoverPage: View {
    enum Filter: String, CaseIterable {
        case songs = "歌曲", artists = "歌手", albums = "专辑", playlists = "歌单"
    }
    @State private var query = ""
    @State private var filter = Filter.songs
    @State private var tracks: [Track] = []
    @State private var artists: [Artist] = []
    @State private var albums: [Album] = []
    @State private var playlists: [Playlist] = []
    @State private var newAlbums: Result<[Album], Error>?
    @State private var loading = false
    @State private var error: String?
    @State private var nextOffset: UInt64?
    @State private var revision = 0

    var body: some View {
        List {
            if query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                NavigationLink(value: Destination.artists) { Label("歌手", systemImage: "music.mic") }
                NavigationLink(value: Destination.albums) { Label("专辑", systemImage: "square.stack") }
                Section("新碟") {
                    switch newAlbums {
                    case let .success(albums):
                        if albums.isEmpty { Text("暂无新碟").foregroundStyle(.secondary) }
                        recordShelf(albums) { album, side in
                            NavigationLink(value: Destination.album(album.mid)) {
                                RecordTile(title: album.name, subtitle: album.credit, cover: album.cover,
                                           releaseYear: album.releaseYear, coverSide: side)
                            }.buttonStyle(.plain)
                        }
                    case let .failure(error):
                        Text(error.localizedDescription).foregroundStyle(.secondary)
                        Button("重试") { Task { await loadNewAlbums() } }
                    case nil: ProgressView()
                    }
                }
            } else {
                Picker("搜索类别", selection: $filter) {
                    ForEach(Filter.allCases, id: \.self) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                if let error {
                    LoadError(message: error) { Task { await search(reset: true) } }
                }
                if loading && resultsEmpty {
                    ProgressView("搜索中")
                } else if error == nil && !loading && resultsEmpty {
                    ContentUnavailableView.search(text: query)
                }
                ForEach(tracks) { TrackRow(track: $0, tracks: tracks) }
                ForEach(artists) { artist in ArtistLink(artist: artist) }
                ForEach(albums) { album in AlbumLink(album: album) }
                ForEach(playlists) { PlaylistLink(playlist: $0) }
                if nextOffset != nil { Button("加载更多") { Task { await search(reset: false) } }.disabled(loading) }
            }
        }
        .navigationTitle("发现")
        .toolbar { AccountToolbar() }
        .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always), prompt: "歌曲、歌手、专辑、歌单")
        .task { if newAlbums == nil { await loadNewAlbums() } }
        .task(id: SearchKey(query: query, filter: filter)) {
            do {
                try await Task.sleep(for: .milliseconds(300))
                await search(reset: true)
            } catch is CancellationError {}
            catch { self.error = error.localizedDescription }
        }
    }

    private struct SearchKey: Hashable { let query: String; let filter: Filter }
    private var resultsEmpty: Bool { tracks.isEmpty && artists.isEmpty && albums.isEmpty && playlists.isEmpty }

    private func loadNewAlbums() async {
        do {
            let page: Page<Album> = try await Core.call("albums", ["query": "", "offset": 0])
            if !Task.isCancelled { newAlbums = .success(Array(page.items.prefix(8))) }
        } catch {
            if !Task.isCancelled { newAlbums = .failure(error) }
        }
    }

    private func search(reset: Bool) async {
        let term = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let category = filter
        revision += 1
        let request = revision
        let offset = reset ? 0 : nextOffset ?? 0
        if reset { tracks = []; artists = []; albums = []; playlists = []; nextOffset = nil }
        guard !term.isEmpty else { loading = false; error = nil; return }
        loading = true
        error = nil
        defer { if revision == request { loading = false } }
        let arguments: [String: any Encodable] = ["query": term, "offset": offset]
        func current() -> Bool { request == revision && term == query.trimmingCharacters(in: .whitespacesAndNewlines) && category == filter && !Task.isCancelled }
        do {
            switch category {
            case .songs:
                let result: Page<Track> = try await Core.call("search", arguments)
                guard current() else { return }
                tracks.append(contentsOf: result.items)
                nextOffset = result.hasMore ? result.nextOffset : nil
            case .artists:
                let result: Page<Artist> = try await Core.call("artists", arguments)
                guard current() else { return }
                artists.append(contentsOf: result.items)
                nextOffset = result.hasMore ? result.nextOffset : nil
            case .albums:
                let result: Page<Album> = try await Core.call("albums", arguments)
                guard current() else { return }
                albums.append(contentsOf: result.items)
                nextOffset = result.hasMore ? result.nextOffset : nil
            case .playlists:
                let result: Page<Playlist> = try await Core.call("searchPlaylists", arguments)
                guard current() else { return }
                playlists.append(contentsOf: result.items)
                nextOffset = result.hasMore ? result.nextOffset : nil
            }
        } catch {
            if current() { self.error = error.localizedDescription }
        }
    }
}

private enum CollectionOrder: String, Hashable {
    case source = "原始顺序"
    case reversed = "反向排列"
    case newest = "时间：从新到旧"
    case oldest = "时间：从旧到新"
    case releaseNewest = "发行时间：从新到旧"
    case releaseOldest = "发行时间：从旧到新"
    case name = "名称 A–Z"
    case nameReverse = "名称 Z–A"

    static let basic: [Self] = [.source, .reversed, .name, .nameReverse]
    static let dates: [Self] = [.newest, .oldest]
    static let release: [Self] = [.releaseNewest, .releaseOldest]

    func sort<Item>(_ items: [Item], name: (Item) -> String,
                    date: (Item) -> UInt64? = { _ in nil },
                    release: (Item) -> UInt64? = { _ in nil }) -> [Item] {
        switch self {
        case .source: return items
        case .reversed: return Array(items.reversed())
        case .name, .nameReverse:
            return items.sorted {
                let comparison = name($0).localizedStandardCompare(name($1))
                return comparison == (self == .name ? .orderedAscending : .orderedDescending)
            }
        case .newest, .oldest, .releaseNewest, .releaseOldest:
            let field = (self == .newest || self == .oldest) ? date : release
            let descending = self == .newest || self == .releaseNewest
            return items.sorted {
                switch (field($0), field($1)) {
                case let (.some(a), .some(b)): return descending ? a > b : a < b
                case (.some, .none): return true
                default: return false
                }
            }
        }
    }
}

private func releaseNumber(_ date: String?) -> UInt64? {
    guard let date, !date.isEmpty else { return nil }
    let parts = date.split(separator: "-")
    guard let year = parts.first.flatMap({ UInt64($0) }), (1000...9999).contains(year) else { return nil }
    let month = parts.count > 1 ? UInt64(parts[1]) ?? 0 : 0
    let day = parts.count > 2 ? UInt64(parts[2]) ?? 0 : 0
    guard month <= 12, day <= 31 else { return nil }
    return year * 10_000 + month * 100 + day
}

private func matches(_ query: String, _ values: String...) -> Bool {
    query.isEmpty || values.contains { $0.localizedStandardContains(query) }
}

private func sortMenu(_ order: Binding<CollectionOrder>, options: [CollectionOrder],
                      original: String = "原始顺序") -> some View {
    Menu("排序", systemImage: "arrow.up.arrow.down") {
        Picker("排序", selection: order) {
            ForEach(options, id: \.self) { option in
                Text(option == .source ? original : option.rawValue).tag(option)
            }
        }
    }
}

struct LibraryPage: View {
    @Environment(ListenerModel.self) private var model
    @State private var error: String?
    @State private var newPlaylist = false
    @State private var name = ""
    @State private var query = ""
    @State private var order = CollectionOrder.source

    var body: some View {
        let term = query.trimmingCharacters(in: .whitespacesAndNewlines)
        List {
            Section {
                if let liked = model.library?.playlists.first(where: { $0.kind == "liked" }),
                   matches(term, liked.name, "喜欢的歌曲") {
                    NavigationLink(value: Destination.playlist(liked)) { Label("喜欢的歌曲", systemImage: "heart.fill") }
                }
                if term.isEmpty {
                    NavigationLink(value: Destination.savedAlbums) { Label("专辑", systemImage: "square.stack") }
                    NavigationLink(value: Destination.savedArtists) { Label("歌手", systemImage: "music.mic") }
                    NavigationLink(value: Destination.recent) { Label("最近播放", systemImage: "clock") }
                    NavigationLink(value: Destination.downloads) { Label("下载与离线", systemImage: "arrow.down.circle") }
                }
            }
            if let library = model.library {
                Section("歌单") {
                    let playlists = order.sort(library.playlists.filter {
                        $0.kind != "liked" && matches(term, $0.name, $0.creator ?? "")
                    }, name: \.name, date: \.activityTime)
                    if playlists.isEmpty { Text(term.isEmpty ? "暂无歌单" : "没有匹配的歌单").foregroundStyle(.secondary) }
                    ForEach(playlists) { PlaylistLink(playlist: $0) }
                    ForEach(library.warnings, id: \.self) { Text($0).font(.footnote).foregroundStyle(.secondary) }
                }
            }
            if let error {
                LoadError(message: error) { Task { await load() } }
            } else if model.library == nil {
                ProgressView("正在读取收藏")
            }
        }
        .navigationTitle("我的收藏")
        .searchable(text: $query, prompt: "查找歌单")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                let hasDates = model.library?.playlists.contains { $0.activityTime != nil } == true
                sortMenu($order, options: CollectionOrder.basic + (hasDates ? CollectionOrder.dates : []),
                         original: "QQ 音乐歌单顺序")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button("新建歌单", systemImage: "plus") { name = ""; newPlaylist = true }
            }
            AccountToolbar()
        }
        .alert("新建歌单", isPresented: $newPlaylist) {
            TextField("歌单名称", text: $name)
            Button("创建") { Task { await model.playlistChange("create", name: name) } }
                .disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            Button("取消", role: .cancel) {}
        }
        .task { if model.library == nil { await load() } }
        .refreshable {
            await load()
            await model.loadLikes(refresh: true)
        }
    }

    private func load() async {
        do { try await model.reloadLibrary(); error = nil }
        catch { if !Task.isCancelled { self.error = error.localizedDescription } }
    }
}

struct TrackRow: View {
    let track: Track
    let tracks: [Track]
    @Environment(Player.self) private var player
    @Environment(ListenerModel.self) private var model

    var body: some View {
        HStack(spacing: 2) {
            playButton
            TrackLikeButton(track: track)
        }
        .listRowInsets(.init(top: 2, leading: 16, bottom: 2, trailing: 16))
        .contextMenu {
            TrackCatalogActions(track: track)
            Button("下一首播放", systemImage: "text.line.first.and.arrowtriangle.forward") { player.add(track, next: true) }
            Button("加入播放队列", systemImage: "text.append") { player.add(track, next: false) }
            Button("下载到 iPhone", systemImage: "arrow.down.circle") { Task { await model.startDownload(track) } }
            if let playlists = model.library?.playlists.filter({ $0.kind == "created" }), !playlists.isEmpty {
                Menu("添加到歌单", systemImage: "text.badge.plus") {
                    ForEach(playlists) { playlist in
                        Button(playlist.name) { Task { await model.playlistChange("add", id: playlist.id, track: track) } }
                    }
                }
            }
        }
    }

    private var playButton: some View {
        Button {
            if player.current?.mid == track.mid { player.toggle() }
            else { player.play(track, in: tracks) }
        } label: {
            HStack(spacing: 9) {
                Cover(url: track.cover, radius: 7).frame(width: 34, height: 34)
                VStack(alignment: .leading, spacing: 1) {
                    TrackTitle(track: track, font: .subheadline.weight(.medium), lineLimit: 1)
                    Text(track.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer(minLength: 6)
                if player.current?.mid == track.mid {
                    Image(systemName: player.playing ? "waveform" : "play.circle")
                        .foregroundStyle(.tint)
                        .accessibilityHidden(true)
                }
            }
            .frame(minHeight: 44)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(track.vip ? "\(track.title)，\(track.artist)，VIP 歌曲" : "\(track.title)，\(track.artist)")
    }
}

struct TrackCatalogActions: View {
    let track: Track
    @Environment(\.catalogRouter) private var router

    var body: some View {
        if let router, let artist = track.artists.first {
            if track.artists.count == 1 {
                Button("查看歌手", systemImage: "music.mic") { router.open(.artist(artist)) }
            } else {
                Menu("查看歌手", systemImage: "music.mic") {
                    ForEach(track.artists) { person in Button(person.name) { router.open(.artist(person)) } }
                }
            }
        }
        if let router, !track.albumMid.isEmpty {
            Button("查看专辑", systemImage: "square.stack") { router.open(.album(track.albumMid)) }
        }
    }
}

struct TrackLikeButton: View {
    let track: Track
    @Environment(ListenerModel.self) private var model

    var body: some View {
        let liked = model.likedMids?.contains(track.mid) == true
        let busy = model.changingLikes.contains(track.mid)
        Button { Task { await model.changeLike(track) } } label: {
            Group {
                if busy { ProgressView().controlSize(.small) }
                else {
                    Image(systemName: liked ? "heart.fill" : "heart")
                        .font(.subheadline.weight(.medium))
                        .foregroundStyle(liked ? AnyShapeStyle(.tint) : AnyShapeStyle(.secondary))
                }
            }
            .frame(width: 44, height: 44)
            .contentShape(.rect)
        }
        .buttonStyle(.borderless)
        .disabled(busy || model.profile?.offline != false)
        .accessibilityLabel(liked ? "取消喜欢，\(track.title)" : "喜欢，\(track.title)")
        .accessibilityHint(model.likeError == nil ? "" : "喜欢状态未能读取，点按重试")
    }
}

struct TrackTitle: View {
    let track: Track
    let font: Font
    let lineLimit: Int
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        HStack(spacing: 5) {
            Text(track.title).font(font).lineLimit(lineLimit)
            if track.vip {
                Text("VIP").font(.caption2.weight(.semibold))
                    .padding(.horizontal, 4)
                    .padding(.vertical, 1)
                    .foregroundStyle(scheme == .dark ? Color(red: 1, green: 0.82, blue: 0.46)
                                     : Color(red: 0.46, green: 0.31, blue: 0.09))
                    .background(.yellow.opacity(scheme == .dark ? 0.19 : 0.17), in: .rect(cornerRadius: 4))
                    .fixedSize()
                    .accessibilityLabel("VIP 歌曲")
            }
        }
        .accessibilityElement(children: .combine)
    }
}

private struct PlaylistLink: View {
    let playlist: Playlist
    var body: some View {
        NavigationLink(value: Destination.playlist(playlist)) {
            HStack(spacing: 12) {
                Cover(url: playlist.cover).frame(width: 48, height: 48)
                VStack(alignment: .leading, spacing: 3) {
                    Text(playlist.name).lineLimit(2)
                    if playlist.count > 0 {
                        Text("\(playlist.count) 首").font(.subheadline).foregroundStyle(.secondary)
                    }
                }
            }
        }
    }
}

private struct ArtistLink: View {
    let artist: Artist
    var body: some View {
        NavigationLink(value: Destination.artist(artist)) {
            HStack(spacing: 12) {
                Cover(url: artist.image, radius: 24).frame(width: 48, height: 48)
                Text(artist.name).lineLimit(2)
            }
        }
    }
}

private struct AlbumLink: View {
    let album: Album
    var body: some View {
        NavigationLink(value: Destination.album(album.mid)) {
            HStack(spacing: 12) {
                Cover(url: album.cover).frame(width: 48, height: 48)
                VStack(alignment: .leading, spacing: 3) {
                    Text(album.name).lineLimit(2)
                    Text(album.credit).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                }
                if let year = album.releaseYear {
                    Spacer(minLength: 4)
                    Text(verbatim: year).font(.footnote.monospacedDigit()).foregroundStyle(.secondary)
                        .fixedSize()
                        .accessibilityLabel("发行年份 \(year)")
                }
            }
        }
    }
}

private struct RecordTile: View {
    let title: String
    let subtitle: String?
    let cover: String
    var releaseYear: String?
    var coverSide: CGFloat?

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Color.clear.aspectRatio(1, contentMode: .fit)
                .frame(width: coverSide, height: coverSide)
                .overlay { Cover(url: cover, radius: 12) }
                .overlay(alignment: .topTrailing) { releaseYearBadge(releaseYear) }
            Text(title).font(.subheadline.weight(.semibold)).lineLimit(2, reservesSpace: true)
            if let subtitle { Text(subtitle).font(.footnote).foregroundStyle(.secondary).lineLimit(1) }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .fixedSize(horizontal: false, vertical: true)
    }
}

@ViewBuilder private func releaseYearBadge(_ year: String?) -> some View {
    if let year {
        Text(verbatim: year).font(.caption.weight(.semibold).monospacedDigit())
            .foregroundStyle(.white)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(.black.opacity(0.66), in: .capsule)
            .padding(8)
            .accessibilityLabel("发行年份 \(year)")
    }
}

private let recordShelfSide: CGFloat = 140

private func recordShelf<Item: Identifiable, Tile: View>(
    _ items: [Item], @ViewBuilder tile: @escaping (Item, CGFloat) -> Tile
) -> some View {
    recordShelf {
        ForEach(items) { item in tile(item, recordShelfSide).frame(width: recordShelfSide) }
    }
}

private func recordShelf<Content: View>(@ViewBuilder content: () -> Content) -> some View {
    ScrollView(.horizontal) {
        LazyHStack(alignment: .top, spacing: 14) {
            content()
        }.scrollTargetLayout()
    }
    .scrollTargetBehavior(.viewAligned)
    .scrollIndicators(.hidden)
}

private struct Header: View {
    let title: String
    let subtitle: String
    let cover: String
    var releaseYear: String?

    var body: some View {
        VStack(spacing: 12) {
            Cover(url: cover, radius: 20)
                .aspectRatio(1, contentMode: .fit)
                .frame(maxWidth: 190)
                .overlay(alignment: .topTrailing) { releaseYearBadge(releaseYear) }
            VStack(spacing: 4) {
                Text(title).font(.title2.bold()).lineLimit(3).accessibilityAddTraits(.isHeader)
                if !subtitle.isEmpty { Text(subtitle).font(.subheadline).foregroundStyle(.secondary).lineLimit(2) }
            }
            .multilineTextAlignment(.center)
            .frame(maxWidth: .infinity)
        }.padding(.vertical, 6)
    }
}

private struct CollectionPlaybackButtons: View {
    let onPlay: (Bool) -> Void
    var preparingShuffle = false
    @Environment(\.dynamicTypeSize) private var typeSize

    init(onPlay: @escaping (Bool) -> Void, preparingShuffle: Bool = false) {
        self.onPlay = onPlay
        self.preparingShuffle = preparingShuffle
    }

    init(tracks: [Track], player: Player) {
        self.init { player.playCollection(tracks, shuffled: $0) }
    }

    var body: some View {
        let layout = typeSize.isAccessibilitySize ? AnyLayout(VStackLayout(spacing: 10)) : AnyLayout(HStackLayout(spacing: 10))
        layout {
            Button { onPlay(false) } label: {
                Label("播放全部", systemImage: "play.fill").frame(maxWidth: .infinity, minHeight: 32)
            }
            .buttonStyle(.glassProminent)
            Button { onPlay(true) } label: {
                Label {
                    Text("随机播放")
                } icon: {
                    if preparingShuffle { ProgressView().controlSize(.small) }
                    else { Image(systemName: "shuffle") }
                }
                .frame(maxWidth: .infinity, minHeight: 32)
            }
            .buttonStyle(.glass)
        }
        .disabled(preparingShuffle)
        .listRowSeparator(.hidden)
    }
}

@MainActor
private func playPagedCollection(
    _ initial: [Track], next: UInt64?, shuffled: Bool, player: Player,
    fetch: (UInt64) async throws -> (tracks: [Track], next: UInt64?)
) async throws {
    if !shuffled { player.playCollection(initial, shuffled: false) }
    let intent = player.reserveCollectionPlayback()
    let revision = player.queueRevision
    var all = initial
    var offset = next
    func current() -> Bool {
        shuffled ? player.collectionPlaybackIsCurrent(intent) : player.queueRevision == revision
    }
    while let start = offset, current() {
        let page = try await fetch(start)
        try Task.checkCancellation()
        guard current() else { return }
        if shuffled { all.append(contentsOf: page.tracks) }
        else if !player.appendPlaylistPage(page.tracks, revision: revision) { return }
        if let next = page.next, next <= start {
            throw CoreError(code: "COLLECTION_PAGE", message: "QQ 音乐未返回下一页歌曲，请稍后重试。")
        }
        offset = page.next
    }
    if shuffled && player.collectionPlaybackIsCurrent(intent) { player.playCollection(all, shuffled: true) }
}

private struct PlaylistDetail: View {
    let playlist: Playlist
    @Environment(Player.self) private var player
    @Environment(ListenerModel.self) private var model
    @State private var tracks: [Track] = []
    @State private var dates: [String: PlaylistTrackDate] = [:]
    @State private var query = ""
    @State private var order = CollectionOrder.source
    @State private var nextOffset: UInt64?
    @State private var loading = false
    @State private var loaded = false
    @State private var preparingShuffle = false
    @State private var error: String?

    private var term: String { query.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var needsCompletePlaylist: Bool { !term.isEmpty || order != .source }
    private var displayed: [Track] {
        order.sort(tracks.filter { matches(term, $0.title, $0.artist, $0.album) }, name: \.title,
                   date: { dates[$0.mid]?.addedAt }, release: { releaseNumber(dates[$0.mid]?.publishedOn) })
    }
    private var sortOptions: [CollectionOrder] {
        CollectionOrder.basic
            + (dates.values.contains { $0.addedAt != nil } ? CollectionOrder.dates : [])
            + (dates.values.contains { $0.publishedOn != nil } ? CollectionOrder.release : [])
    }

    var body: some View {
        let shown = displayed
        let completing = needsCompletePlaylist && nextOffset != nil
        List {
            Header(title: playlist.name, subtitle: playlist.creator ?? "", cover: playlist.cover)
                .listRowSeparator(.hidden)
            if !shown.isEmpty {
                CollectionPlaybackButtons(onPlay: { shuffled in
                    if needsCompletePlaylist { player.playCollection(shown, shuffled: shuffled) }
                    else {
                        if shuffled { preparingShuffle = true }
                        Task { await playAll(shuffled: shuffled) }
                    }
                }, preparingShuffle: preparingShuffle)
                .disabled(completing)
            }
            ForEach(shown) { track in
                TrackRow(track: track, tracks: shown)
                    .disabled(completing)
                    .swipeActions {
                    if playlist.kind == "liked" || playlist.kind == "created" {
                        Button("移出歌单", systemImage: "minus.circle", role: .destructive) {
                            Task {
                                if playlist.kind == "liked" {
                                    await model.changeLike(track, to: false)
                                } else {
                                    await model.playlistChange("remove", id: playlist.id, track: track)
                                    await load(reset: true)
                                }
                            }
                        }
                    }
                }
            }
            if let error { LoadError(message: error) { Task { await load(reset: !loaded) } } }
            else if loading || completing { ProgressView(completing ? "正在读取完整歌单" : "正在加载") }
            else if loaded && shown.isEmpty {
                ContentUnavailableView(term.isEmpty ? "暂无歌曲" : "没有匹配的歌曲", systemImage: "music.note")
            }
            if nextOffset != nil && error == nil && !completing {
                Button("加载更多") { Task { await load(reset: false) } }.disabled(loading)
            }
        }
        .listStyle(.plain)
        .navigationTitle(playlist.name)
        .navigationBarTitleDisplayMode(.inline)
        .searchable(text: $query, prompt: "查找歌名、歌手或专辑")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                sortMenu($order, options: sortOptions, original: "QQ 音乐歌单顺序")
            }
            if ["public", "collected"].contains(playlist.kind) {
                ToolbarItem(placement: .topBarTrailing) {
                    let saved = model.library?.playlists.contains { $0.id == playlist.id && $0.kind == "collected" } ?? (playlist.kind == "collected")
                    Button(saved ? "取消收藏" : "收藏歌单", systemImage: saved ? "heart.fill" : "heart") {
                        Task { await model.playlistChange(saved ? "uncollect" : "collect", id: playlist.id) }
                    }
                }
            }
        }
        .task {
            if !loaded { await load(reset: true) }
            else if needsCompletePlaylist && nextOffset != nil { await load(reset: false) }
        }
        .onChange(of: needsCompletePlaylist) { _, complete in
            if complete && nextOffset != nil { Task { await load(reset: false) } }
        }
        .onChange(of: model.likedRevision) { _, _ in
            if playlist.kind == "liked" && loaded { Task { await load(reset: true) } }
        }
        .refreshable { await load(reset: true) }
    }

    private func load(reset: Bool) async {
        guard !loading else { return }
        loading = true
        error = nil
        defer { loading = false }
        do {
            var offset = reset ? 0 : nextOffset ?? 0
            repeat {
                let response: PlaylistPage = try await Core.call("playlist", ["id": playlist.id, "offset": offset])
                try Task.checkCancellation()
                if offset == 0 {
                    tracks = response.tracks
                    dates = response.dates ?? [:]
                } else {
                    tracks += response.tracks
                    dates.merge(response.dates ?? [:], uniquingKeysWith: { first, _ in first })
                }
                nextOffset = response.hasMore ? response.nextOffset : nil
                loaded = true
                offset = response.nextOffset
            } while needsCompletePlaylist && nextOffset != nil
        } catch is CancellationError {}
        catch { self.error = error.localizedDescription }
    }

    private func playAll(shuffled: Bool) async {
        defer { if shuffled { preparingShuffle = false } }
        do {
            try await playPagedCollection(tracks, next: nextOffset, shuffled: shuffled, player: player) { start in
                let response: PlaylistPage = try await Core.call("playlist", ["id": playlist.id, "offset": start])
                return (response.tracks, response.hasMore ? response.nextOffset : nil)
            }
        } catch is CancellationError {}
        catch { model.error = error.localizedDescription }
    }
}

private struct AlbumPage: View {
    let mid: String
    @Environment(Player.self) private var player
    @Environment(ListenerModel.self) private var model
    @State private var detail: AlbumDetail?
    @State private var error: String?
    @State private var showDescription = false

    var body: some View {
        List {
            if let detail {
                Header(title: detail.album.name, subtitle: detail.album.credit, cover: detail.album.cover,
                       releaseYear: detail.album.releaseYear)
                    .listRowSeparator(.hidden)
                if !detail.tracks.isEmpty {
                    CollectionPlaybackButtons(tracks: detail.tracks, player: player)
                }
                ForEach(detail.tracks) { TrackRow(track: $0, tracks: detail.tracks) }
                if detail.tracks.isEmpty { ContentUnavailableView("暂无曲目", systemImage: "music.note") }
                if !detail.description.isEmpty {
                    DisclosureGroup("专辑介绍", isExpanded: $showDescription) {
                        Text(detail.description).font(.subheadline).foregroundStyle(.secondary)
                    }
                }
            } else if let error { LoadError(message: error) { Task { await load() } } }
            else { ProgressView("正在加载专辑") }
        }
        .listStyle(.plain)
        .navigationTitle(detail?.album.name ?? "专辑")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if let detail {
                ToolbarItem(placement: .topBarTrailing) {
                    let saved = model.saved?.albums.contains { $0.mid == mid } == true
                    Button(saved ? "取消收藏" : "收藏专辑", systemImage: saved ? "heart.fill" : "heart") {
                        Task { await model.save(detail.album) }
                    }
                    .disabled(model.saved == nil)
                }
            }
        }
        .task { await load() }
    }

    private func load() async {
        error = nil
        do {
            let value: AlbumDetail = try await Core.call("album", ["mid": mid])
            try Task.checkCancellation()
            detail = value
            if model.saved == nil { try await model.reloadSaved() }
        } catch is CancellationError {}
        catch {
            if detail == nil { self.error = error.localizedDescription }
            else { model.error = error.localizedDescription }
        }
    }
}

private struct ArtistPage: View {
    enum Section: String, CaseIterable {
        case songs = "歌曲", albums = "专辑", about = "简介"
    }
    let artist: Artist
    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player
    @State private var detail: ArtistDetail?
    @State private var section = Section.songs
    @State private var order = "popular"
    @State private var tracks: [Track] = []
    @State private var albums: [Album] = []
    @State private var nextOffset: UInt64?
    @State private var loading = true
    @State private var error: String?
    @State private var revision = 0
    @State private var preparingShuffle = false

    var body: some View {
        List {
            Header(title: artist.name, subtitle: detail.map { "\($0.totalSongs) 首歌曲 · \($0.totalAlbums) 张专辑" } ?? "", cover: artist.image)
                .listRowSeparator(.hidden)
            Picker("歌手内容", selection: $section) {
                ForEach(Section.allCases, id: \.self) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented)
            .listRowSeparator(.hidden)
            if section == .songs {
                Picker("排序", selection: $order) {
                    Text("按热度").tag("popular")
                    Text("按时间").tag("newest")
                }.listRowSeparator(.hidden)
                if !tracks.isEmpty {
                    CollectionPlaybackButtons(onPlay: { shuffled in
                        if shuffled { preparingShuffle = true }
                        Task { await playSongs(shuffled: shuffled) }
                    }, preparingShuffle: preparingShuffle)
                    .disabled(loading)
                }
                ForEach(tracks) { TrackRow(track: $0, tracks: tracks) }
            } else if section == .albums {
                ForEach(albums) { AlbumLink(album: $0) }
            } else if let detail {
                Text(detail.description.isEmpty ? "暂无介绍" : detail.description)
                    .font(.body).foregroundStyle(.secondary)
                    .listRowSeparator(.hidden)
            }
            if let error { LoadError(message: error) { Task { await load(reset: true) } } }
            else if loading { ProgressView() }
            else if (section == .songs && tracks.isEmpty) || (section == .albums && albums.isEmpty) {
                ContentUnavailableView("暂无内容", systemImage: "music.note")
            }
            if section != .about && nextOffset != nil {
                Button("加载更多") { Task { await load(reset: false) } }.disabled(loading)
            }
        }
        .listStyle(.plain)
        .navigationTitle(artist.name)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                let followed = model.saved?.artists.contains { $0.mid == artist.mid } == true
                Button(followed ? "取消关注" : "关注歌手", systemImage: followed ? "person.crop.circle.badge.checkmark" : "person.crop.circle.badge.plus") {
                    Task { await model.save(artist) }
                }.disabled(model.saved == nil)
            }
        }
        .task(id: section.rawValue + order) { await load(reset: true) }
    }

    private func playSongs(shuffled: Bool) async {
        let sort = order
        defer { if shuffled { preparingShuffle = false } }
        do {
            try await playPagedCollection(tracks, next: nextOffset, shuffled: shuffled, player: player) { offset in
                let page: Page<Track> = try await Core.call("artistSongs", ["mid": artist.mid, "offset": offset, "order": sort])
                return (page.items, page.hasMore ? page.nextOffset : nil)
            }
        } catch is CancellationError {}
        catch { model.error = error.localizedDescription }
    }

    private func load(reset: Bool) async {
        let selected = section
        let sort = order
        let offset = reset ? 0 : nextOffset ?? 0
        revision += 1
        let request = revision
        loading = true
        error = nil
        if reset { tracks = []; albums = []; nextOffset = nil }
        defer { if revision == request { loading = false } }
        func current() -> Bool { !Task.isCancelled && request == revision && section == selected && order == sort }
        do {
            if detail == nil {
                let value: ArtistDetail = try await Core.call("artist", ["mid": artist.mid])
                guard current() else { return }
                detail = value
            }
            switch selected {
            case .songs:
                let page: Page<Track> = try await Core.call("artistSongs", ["mid": artist.mid, "offset": offset, "order": sort])
                guard current() else { return }
                tracks.append(contentsOf: page.items)
                nextOffset = page.hasMore ? page.nextOffset : nil
            case .albums:
                let page: Page<Album> = try await Core.call("artistAlbums", ["mid": artist.mid, "offset": offset])
                guard current() else { return }
                albums.append(contentsOf: page.items)
                nextOffset = page.hasMore ? page.nextOffset : nil
            case .about: break
            }
            if model.saved == nil { try await model.reloadSaved() }
        } catch {
            if current() { self.error = error.localizedDescription }
        }
    }
}

private struct DirectoryPage: View {
    let artists: Bool
    @State private var people: [Artist] = []
    @State private var records: [Album] = []
    @State private var nextOffset: UInt64?
    @State private var loading = false
    @State private var error: String?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                if artists {
                    ForEach(people) { ArtistLink(artist: $0) }
                } else {
                    AlbumGrid(albums: records)
                }
                if let error { LoadError(message: error) { Task { await load() } } }
                else if loading { ProgressView() }
                else if people.isEmpty && records.isEmpty { ContentUnavailableView("暂无内容", systemImage: "music.note") }
                if nextOffset != nil && error == nil {
                    Button("加载更多") { Task { await load() } }.disabled(loading)
                }
            }
            .padding(20)
        }
        .navigationTitle(artists ? "歌手" : "专辑")
        .task { if people.isEmpty && records.isEmpty { await load() } }
    }

    private func load() async {
        guard !loading else { return }
        loading = true
        error = nil
        defer { loading = false }
        let parameters: [String: any Encodable] = ["query": "", "offset": nextOffset ?? 0]
        do {
            if artists {
                let page: Page<Artist> = try await Core.call("artists", parameters)
                try Task.checkCancellation()
                people.append(contentsOf: page.items)
                nextOffset = page.hasMore ? page.nextOffset : nil
            } else {
                let page: Page<Album> = try await Core.call("albums", parameters)
                try Task.checkCancellation()
                records.append(contentsOf: page.items)
                nextOffset = page.hasMore ? page.nextOffset : nil
            }
        } catch is CancellationError {}
        catch { self.error = error.localizedDescription }
    }
}

private struct SavedPage: View {
    let artists: Bool
    @Environment(ListenerModel.self) private var model
    @State private var error: String?
    @State private var query = ""
    @State private var order = CollectionOrder.source
    private var sortOptions: [CollectionOrder] {
        let savedTimes = artists ? model.saved?.artistSavedAt : model.saved?.albumSavedAt
        return CollectionOrder.basic + (savedTimes?.isEmpty == false ? CollectionOrder.dates : [])
            + (artists ? [] : CollectionOrder.release)
    }

    var body: some View {
        let term = query.trimmingCharacters(in: .whitespacesAndNewlines)
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                if let saved = model.saved {
                    if artists {
                        let items = order.sort(saved.artists.filter { matches(term, $0.name) }, name: \.name,
                                               date: { saved.artistSavedAt?[$0.mid] })
                        if items.isEmpty { ContentUnavailableView(term.isEmpty ? "还没有关注的歌手" : "没有匹配的歌手", systemImage: "music.mic") }
                        ForEach(items) { ArtistLink(artist: $0) }
                    } else {
                        let items = order.sort(saved.albums.filter { matches(term, $0.name, $0.credit) },
                                               name: \.name, date: { saved.albumSavedAt?[$0.mid] },
                                               release: { releaseNumber($0.publishDate) })
                        if items.isEmpty { ContentUnavailableView(term.isEmpty ? "还没有收藏的专辑" : "没有匹配的专辑", systemImage: "square.stack") }
                        AlbumGrid(albums: items)
                    }
                }
                if let error { LoadError(message: error) { Task { await load() } } }
                else if model.saved == nil { ProgressView("正在同步收藏") }
            }.padding(20)
        }
        .navigationTitle(artists ? "关注的歌手" : "收藏的专辑")
        .searchable(text: $query, prompt: artists ? "查找歌手" : "查找专辑或歌手")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                sortMenu($order, options: sortOptions,
                         original: artists ? "QQ 音乐关注顺序" : "QQ 音乐收藏顺序")
            }
        }
        .task { await load() }
        .refreshable { await load() }
    }

    private func load() async {
        error = nil
        do { try await model.reloadSaved() }
        catch { if !Task.isCancelled { self.error = error.localizedDescription } }
    }
}

private struct AlbumGrid: View {
    let albums: [Album]
    @ScaledMetric(relativeTo: .subheadline) private var columnWidth = 128.0

    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: columnWidth), spacing: 16)], alignment: .leading, spacing: 20) {
            ForEach(albums) { album in
                NavigationLink(value: Destination.album(album.mid)) {
                    RecordTile(title: album.name, subtitle: album.credit, cover: album.cover, releaseYear: album.releaseYear)
                }
                .buttonStyle(.plain)
            }
        }
    }
}

private struct LocalTracks: View {
    let title: String
    let downloaded: Bool
    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player
    @State private var tracks: [Track]?
    @State private var error: String?
    @State private var query = ""
    @State private var order = CollectionOrder.source

    var body: some View {
        let term = query.trimmingCharacters(in: .whitespacesAndNewlines)
        List {
            if downloaded && !model.downloadTasks.isEmpty {
                Section("正在下载") {
                    ForEach(model.downloadTasks) { track in
                        HStack {
                            ProgressView()
                            TrackTitle(track: track, font: .body, lineLimit: 1)
                            Spacer()
                            Button("取消", systemImage: "xmark.circle") { Task { await model.cancelDownload(track) } }
                                .labelStyle(.iconOnly)
                        }
                    }
                }
            }
            if let error { LoadError(message: error) { Task { await load() } } }
            else if let tracks {
                let shown = order.sort(tracks.filter { matches(term, $0.title, $0.artist, $0.album) }, name: \.title)
                if !shown.isEmpty {
                    CollectionPlaybackButtons(tracks: shown, player: player)
                }
                ForEach(shown) { track in
                    TrackRow(track: track, tracks: shown)
                        .swipeActions {
                        if downloaded {
                            Button("移除下载", systemImage: "trash", role: .destructive) { Task { await model.removeDownload(track) } }
                        }
                    }
                }
                if shown.isEmpty && (!downloaded || model.downloadTasks.isEmpty || !term.isEmpty) {
                    ContentUnavailableView(term.isEmpty ? (downloaded ? "暂无下载" : "暂无播放记录") : "没有匹配的歌曲",
                                           systemImage: downloaded ? "arrow.down.circle" : "clock")
                }
            } else { ProgressView() }
        }
        .listStyle(.plain)
        .navigationTitle(title)
        .searchable(text: $query, prompt: "查找歌名、歌手或专辑")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                sortMenu($order, options: CollectionOrder.basic,
                         original: downloaded ? "最近下载优先" : "最近播放优先")
            }
        }
        .task(id: downloaded ? model.downloadsRevision : 0) { await load() }
        .refreshable { await load() }
    }

    private func load() async {
        error = nil
        do {
            if downloaded {
                let records: [Download] = try await Core.call("downloads")
                tracks = records.map(\.track)
            } else {
                tracks = try await Core.call("history")
            }
        } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
    }
}
