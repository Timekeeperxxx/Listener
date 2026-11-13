import SwiftUI
import Observation

@MainActor @Observable
final class ListenerModel {
    var profile: Profile?
    var checkingSession = true
    var sessionError: String?
    var library: Library?
    private(set) var likedMids: Set<String>?
    private(set) var changingLikes: Set<String> = []
    private(set) var likeError: String?
    private(set) var likedRevision = 0
    var saved: Saved?
    var home = HomeFeed()
    var settings: NativeSettings?
    var downloadTasks: [Track] = []
    var downloadsRevision = 0
    var error: String?
    var showingSettings = false
    var showingPlayer = false
    @ObservationIgnored private var libraryRevision = 0
    @ObservationIgnored private var accountRevision = 0
    @ObservationIgnored private var likesLoad: Task<Void, Never>?

    func restore(player: Player) async {
        checkingSession = true
        sessionError = nil
        defer { checkingSession = false }
        do {
            try Core.start()
            let profile: Profile? = try await Core.call("session")
            await connect(profile, player: player)
        } catch {
            sessionError = error.localizedDescription
        }
    }

    func connect(_ newProfile: Profile?, player: Player) async {
        libraryRevision += 1
        accountRevision += 1
        likesLoad?.cancel()
        likesLoad = nil
        likedMids = nil
        changingLikes = []
        likeError = nil
        library = nil
        saved = nil
        home.cancel()
        home = HomeFeed()
        downloadTasks = []
        sessionError = nil
        profile = newProfile
        await player.connect(newProfile?.uin)
    }

    func reloadLibrary() async throws {
        guard let owner = profile?.uin else { return }
        libraryRevision += 1
        let request = libraryRevision
        let value: Library = try await Core.call("library")
        if profile?.uin == owner && request == libraryRevision { library = value }
    }

    func reloadSaved() async throws {
        guard let owner = profile?.uin else { return }
        let value: Saved = try await Core.call("saved")
        if profile?.uin == owner { saved = value }
    }

    func loadLikes(refresh: Bool = false) async {
        guard profile?.offline == false, refresh || likedMids == nil else { return }
        if refresh && !changingLikes.isEmpty { return }
        if let likesLoad { await likesLoad.value; return }
        let revision = accountRevision
        let task = Task {
            do {
                let mids: Set<String> = try await Core.call("likedMids")
                guard revision == accountRevision else { return }
                likedMids = mids
                likeError = nil
            } catch {
                if revision == accountRevision { likeError = error.localizedDescription }
            }
        }
        likesLoad = task
        await task.value
        guard revision == accountRevision else { return }
        likesLoad = nil
    }

    func changeLike(_ track: Track, to requested: Bool? = nil) async {
        guard profile?.offline == false, changingLikes.insert(track.mid).inserted else { return }
        let revision = accountRevision
        defer { if revision == accountRevision { changingLikes.remove(track.mid) } }
        if let likesLoad { await likesLoad.value }
        else if likedMids == nil { await loadLikes() }
        guard revision == accountRevision else { return }
        guard let likedMids else {
            error = likeError
            return
        }
        let isLiked = likedMids.contains(track.mid)
        let next = requested ?? !isLiked
        guard next != isLiked else { return }
        do {
            let value: Library = try await Core.call("playlistChange", [
                "action": next ? "add" : "remove", "id": "liked", "track": track,
            ])
            guard revision == accountRevision else { return }
            libraryRevision += 1
            library = value
            if next { self.likedMids?.insert(track.mid) }
            else { self.likedMids?.remove(track.mid) }
            likedRevision += 1
        } catch {
            if revision == accountRevision { self.error = error.localizedDescription }
        }
    }

    func reloadSettings() async {
        do { settings = try await Core.call("settings") }
        catch { self.error = error.localizedDescription }
    }

    func playlistChange(_ action: String, id: String? = nil, name: String? = nil, track: Track? = nil) async {
        guard let owner = profile?.uin else { return }
        do {
            let value: Library = try await Core.call("playlistChange", ["action": action, "id": id, "name": name, "track": track])
            if profile?.uin == owner {
                libraryRevision += 1
                library = value
            }
        } catch {
            if profile?.uin == owner { self.error = error.localizedDescription }
        }
    }

    func save(_ artist: Artist) async {
        guard let owner = profile?.uin else { return }
        let active = saved?.artists.contains { $0.mid == artist.mid } == true
        do {
            let value: Saved = try await Core.call("saveArtist", ["artist": artist, "saved": !active])
            if profile?.uin == owner { saved = value }
        } catch { if profile?.uin == owner { self.error = error.localizedDescription } }
    }

    func save(_ album: Album) async {
        guard let owner = profile?.uin else { return }
        let active = saved?.albums.contains { $0.mid == album.mid } == true
        do {
            let value: Saved = try await Core.call("saveAlbum", ["album": album, "saved": !active])
            if profile?.uin == owner { saved = value }
        } catch { if profile?.uin == owner { self.error = error.localizedDescription } }
    }

    func startDownload(_ track: Track) async {
        guard let owner = profile?.uin, !downloadTasks.contains(where: { $0.mid == track.mid }) else { return }
        downloadTasks.append(track)
        defer { if profile?.uin == owner { downloadTasks.removeAll { $0.mid == track.mid } } }
        do {
            let _: Download = try await Core.call("download", ["track": track])
            if profile?.uin == owner { downloadsRevision += 1 }
        } catch let failure as CoreError where failure.code == "DOWNLOAD_CANCELLED" {
            // The user canceled this request.
        } catch { if profile?.uin == owner { self.error = error.localizedDescription } }
    }

    func cancelDownload(_ track: Track) async {
        do { try await Core.perform("cancelDownload", ["mid": track.mid]) }
        catch { self.error = error.localizedDescription }
    }

    func removeDownload(_ track: Track) async {
        guard let owner = profile?.uin else { return }
        do {
            try await Core.perform("removeDownload", ["mid": track.mid])
            if profile?.uin == owner { downloadsRevision += 1 }
        } catch { if profile?.uin == owner { self.error = error.localizedDescription } }
    }
}

@main
struct ListenerNativeApp: App {
    @State private var model = ListenerModel()
    @State private var player = Player()
    @AppStorage("listener.native.appearance") private var appearance = "system"
    @AppStorage("listener.native.qqAutoMix") private var qqAutoMix = true
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            Group {
                if model.checkingSession || model.profile != nil {
                    ListenerTabs()
                        .id(model.profile?.uin)
                        .allowsHitTesting(!model.checkingSession)
                } else {
                    AccountGate()
                }
            }
            .environment(model)
            .environment(player)
            .preferredColorScheme(appearance == "system" ? nil : appearance == "dark" ? .dark : .light)
            .modifier(ListenerAccent())
            .task {
                await model.restore(player: player)
                await model.reloadSettings()
            }
            .onOpenURL { Core.resume($0) }
            .onChange(of: qqAutoMix, initial: true) { _, enabled in player.configureAutoMix(enabled) }
            .onChange(of: phase) { _, next in
                if next == .background { player.persist() }
            }
            .alert("Listener", isPresented: Binding(
                get: { model.error != nil || player.error != nil },
                set: { if !$0 { model.error = nil; player.error = nil } }
            )) {
                Button("好", role: .cancel) { model.error = nil; player.error = nil }
            } message: {
                Text(model.error ?? player.error ?? "")
            }
        }
    }
}

private struct ListenerAccent: ViewModifier {
    @Environment(\.colorScheme) private var scheme
    func body(content: Content) -> some View {
        content.tint(scheme == .dark ? Color(red: 1, green: 0.65, blue: 0.35) : Color(red: 0.12, green: 0.34, blue: 0.88))
    }
}

private struct ListenerTabs: View {
    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player

    var body: some View {
        @Bindable var model = model
        Group {
            if #available(iOS 26.1, *) {
                tabs.tabViewBottomAccessory(isEnabled: player.current != nil) { MiniPlayer() }
            } else {
                tabs.tabViewBottomAccessory { MiniPlayer() }
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        .sheet(isPresented: $model.showingPlayer) { NowPlaying() }
        .sheet(isPresented: $model.showingSettings) { SettingsPage() }
        .task(id: model.checkingSession) {
            if !model.checkingSession { await model.loadLikes() }
        }
        .task(id: model.checkingSession) {
            guard !model.checkingSession, model.profile?.offline == false, model.library == nil else { return }
            do { try await model.reloadLibrary() }
            catch { if !Task.isCancelled { model.error = error.localizedDescription } }
        }
    }

    private var tabs: some View {
        TabView {
            Tab("推荐", systemImage: "house") {
                CatalogNavigation { HomePage() }
            }
            Tab("发现", systemImage: "safari") {
                CatalogNavigation { DiscoverPage() }
            }
            Tab("我的收藏", systemImage: "heart") {
                CatalogNavigation { LibraryPage() }
            }
        }
    }
}

struct AccountToolbar: ToolbarContent {
    @Environment(ListenerModel.self) private var model

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Button {
                model.showingSettings = true
            } label: {
                Image(systemName: "person.crop.circle")
                    .accessibilityLabel("账户与设置")
            }
        }
    }
}

struct Cover: View {
    let url: String
    var radius: CGFloat = 10

    var body: some View {
        Color(uiColor: .secondarySystemFill)
            .overlay {
                AsyncImage(url: URL(string: url)) { phase in
                    if let image = phase.image {
                        image.resizable().scaledToFill()
                    } else {
                        Image(systemName: "music.note")
                            .font(.body.weight(.medium))
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .clipShape(.rect(cornerRadius: radius))
            .accessibilityHidden(true)
    }
}

struct LoadError: View {
    let message: String
    var retry: (() -> Void)?

    var body: some View {
        ContentUnavailableView {
            Label("加载失败", systemImage: "exclamationmark.circle")
        } description: {
            Text(message)
        } actions: {
            if let retry { Button("重试", action: retry).buttonStyle(.bordered) }
        }
    }
}
