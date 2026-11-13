import SwiftUI
import AVFoundation
import UIKit

struct MiniPlayer: View {
    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement

    var body: some View {
        if let track = player.current {
            HStack(spacing: 8) {
                Button {
                    model.showingPlayer = true
                } label: {
                    HStack(spacing: 9) {
                        Cover(url: track.cover, radius: 7).frame(width: 34, height: 34)
                        VStack(alignment: .leading, spacing: 2) {
                            TrackTitle(track: track, font: .subheadline.weight(.semibold), lineLimit: 1)
                            if placement != .inline {
                                Text(track.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                            }
                        }
                        Spacer(minLength: 0)
                    }
                    .frame(minHeight: 44)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(track.vip ? "打开正在播放，\(track.title)，VIP 歌曲" : "打开正在播放，\(track.title)")
                transportButton(player.playing || player.loading ? "暂停" : "播放",
                                player.playing || player.loading ? "pause.fill" : "play.fill") { player.toggle() }
                if placement != .inline {
                    transportButton("下一首", "forward.fill") { player.advance() }
                }
            }
            .padding(.leading, 12)
            .padding(.trailing, 6)
        }
    }
}

struct NowPlaying: View {
    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    @State private var showLyrics = false
    @State private var lyricControlsVisible = false
    @State private var lyricBrowsing = false
    @State private var scrubbing = false
    @State private var showModes = false
    @State private var showQueue = false
    @State private var keepControlsUntil = ContinuousClock.now
    @State private var hideControlsTask: Task<Void, Never>?
    @State private var previousIdleTimer: Bool?
    @State private var router = CatalogRouter()

    var body: some View {
        @Bindable var router = router
        NavigationStack(path: $router.path) {
            GeometryReader { geometry in
                if let track = player.current {
                    Group {
                        if showLyrics {
                            lyricPage(track, width: geometry.size.width)
                        } else {
                            coverPage(track, size: geometry.size)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .background {
                        if showLyrics {
                            PlayerBackdrop(track: track, tone: player.coverTone,
                                           animated: model.settings?.animatedBackground != false && !reduceMotion)
                        } else {
                            CoverBackdrop(image: player.coverArtwork, tone: player.coverTone)
                        }
                    }
                    .environment(\.colorScheme, .dark)
                }
            }
            .toolbar(.hidden, for: .navigationBar)
            .accessibilityAction(.escape) { dismiss() }
            .onChange(of: player.current?.mid) { _, _ in
                lyricBrowsing = false
                if showLyrics { holdControls() }
            }
            .catalogDestinations()
        }
        .environment(\.catalogRouter, router)
        .sheet(isPresented: $showQueue) { QueuePage() }
        .onChange(of: showQueue) { _, _ in holdControls() }
        .onChange(of: showModes) { _, _ in holdControls() }
        .onChange(of: showLyrics) { _, _ in updateIdleTimer() }
        .onChange(of: scenePhase) { _, _ in updateIdleTimer() }
        .onDisappear {
            hideControlsTask?.cancel()
            restoreIdleTimer()
        }
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
        .presentationContentInteraction(.scrolls)
    }

    private func lyricPage(_ track: Track, width: CGFloat) -> some View {
        let contentWidth = min(520, width - 48)
        return VStack(spacing: 6) {
            lyricHeader(track)
            LyricPanel(track: track, ink: player.coverTone.ink, browsingChanged: setLyricBrowsing)
                .id(track.mid)
                .overlay(alignment: .bottom) {
                    ZStack {
                        if lyricControlsVisible {
                            lyricControls(compact: contentWidth - 28 < 328)
                                .transition(reduceMotion ? .opacity : .offset(y: 12).combined(with: .opacity))
                        }
                    }
                    .animation(reduceMotion ? nil : lyricFollowMotion, value: lyricControlsVisible)
                }
        }
        .frame(maxWidth: 520)
        .padding(.horizontal, 24)
        .padding(.top, 20)
        .padding(.bottom, 8)
    }

    private func coverPage(_ track: Track, size: CGSize) -> some View {
        let coverHeight = min(size.width, max(size.width * 0.70, size.height - 312))
        return ScrollView {
            VStack(spacing: 0) {
                Button { setLyrics(true) } label: {
                    coverArt(track, width: size.width, height: coverHeight)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("打开歌词")
                VStack(spacing: 8) {
                    Spacer(minLength: 4)
                    CurrentLyricPreview()
                    Spacer(minLength: 4)
                    HStack(spacing: 2) {
                        SongMetadata(track: track)
                        TrackLikeButton(track: track)
                        trackMenu(track).tint(.white)
                    }
                    Scrubber().tint(.white)
                    controls(compact: size.width - 44 < 328).tint(.white)
                }
                .frame(maxWidth: 520)
                .padding(.horizontal, 22)
                .padding(.vertical, 12)
            }
            .frame(minHeight: size.height)
        }
        .scrollBounceBehavior(.basedOnSize)
    }

    private func coverArt(_ track: Track, width: CGFloat, height: CGFloat) -> some View {
        let fade = LinearGradient(stops: [
            .init(color: .black, location: 0),
            .init(color: .black, location: 0.61),
            .init(color: .black.opacity(0.82), location: 0.73),
            .init(color: .black.opacity(0.32), location: 0.89),
            .init(color: .clear, location: 1),
        ], startPoint: .top, endPoint: .bottom)
        return Group {
            if let image = player.coverArtwork { Image(uiImage: image).resizable().scaledToFill() }
            else { Cover(url: track.cover, radius: 0) }
        }
        .frame(width: width, height: height)
        .clipped()
        .overlay(alignment: .top) {
            LinearGradient(colors: [.black.opacity(0.50), .clear], startPoint: .top, endPoint: .bottom)
                .frame(height: 36)
                .allowsHitTesting(false)
        }
        .mask(fade)
        .contentShape(.rect)
    }

    private func updateIdleTimer() {
        if showLyrics && scenePhase == .active {
            if previousIdleTimer == nil { previousIdleTimer = UIApplication.shared.isIdleTimerDisabled }
            UIApplication.shared.isIdleTimerDisabled = true
        } else {
            restoreIdleTimer()
        }
    }

    private func restoreIdleTimer() {
        if let previousIdleTimer {
            UIApplication.shared.isIdleTimerDisabled = previousIdleTimer
            self.previousIdleTimer = nil
        }
    }

    private func setLyrics(_ visible: Bool) {
        withAnimation(reduceMotion ? nil : .easeInOut(duration: 0.24)) {
            lyricControlsVisible = visible
            lyricBrowsing = false
            showLyrics = visible
        }
        if visible { holdControls(for: .milliseconds(1600)) }
        else { hideControlsTask?.cancel() }
    }

    private func setLyricBrowsing(_ browsing: Bool) {
        lyricBrowsing = browsing
        if browsing { lyricControlsVisible = true }
        scheduleControlsHide()
    }

    private func holdControls(for duration: Duration = .milliseconds(1400)) {
        guard showLyrics else { return }
        keepControlsUntil = .now.advanced(by: duration)
        lyricControlsVisible = true
        scheduleControlsHide()
    }

    private func scheduleControlsHide() {
        hideControlsTask?.cancel()
        guard showLyrics, !lyricBrowsing, !scrubbing, !showQueue, !showModes else { return }
        if keepControlsUntil <= .now {
            lyricControlsVisible = false
        } else {
            hideControlsTask = Task {
                do { try await Task.sleep(until: keepControlsUntil, clock: .continuous) }
                catch { return }
                lyricControlsVisible = false
            }
        }
    }

    private func performControl(_ action: () -> Void) {
        holdControls()
        action()
    }

    private func lyricHeader(_ track: Track) -> some View {
        Button { setLyrics(false) } label: {
            HStack(spacing: 10) {
                Cover(url: track.cover, radius: 8).frame(width: 40, height: 40)
                SongMetadata(track: track, compact: true)
            }
            .frame(minHeight: 44)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityHint("返回播放控制")
        .accessibilityIdentifier("return-to-player")
    }

    private func trackMenu(_ track: Track) -> some View {
        Menu("更多", systemImage: "ellipsis") {
            if !track.albumMid.isEmpty {
                Button("查看专辑", systemImage: "square.stack") { router.open(.album(track.albumMid)) }
            }
            ForEach(track.artists) { artist in
                Button(artist.name, systemImage: "music.mic") { router.open(.artist(artist)) }
            }
        }
        .labelStyle(.iconOnly)
        .buttonStyle(.glass)
        .frame(minWidth: 44, minHeight: 44)
    }

    private func lyricControls(compact: Bool) -> some View {
        VStack(spacing: 7) {
            Scrubber(onInteraction: { active in
                scrubbing = active
                holdControls()
            })
            controls(compact: compact)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(.ultraThinMaterial, in: .rect(cornerRadius: 24))
        .tint(player.coverTone.ink)
    }

    private func controls(compact: Bool) -> some View {
        GlassEffectContainer(spacing: 10) {
            HStack(spacing: 0) {
                if compact {
                    Button { showModes = true } label: {
                        Image(systemName: "ellipsis").font(.body.weight(.semibold))
                            .frame(width: 44, height: 44)
                    }
                    .accessibilityLabel("播放模式")
                    .buttonStyle(.plain)
                    .popover(isPresented: $showModes) {
                        VStack(alignment: .leading, spacing: 8) {
                            Button(player.shuffle ? "关闭随机播放" : "随机播放", systemImage: "shuffle") {
                                performControl { player.toggleShuffle() }
                            }
                            Button(player.repeatMode.title, systemImage: player.repeatMode == .one ? "repeat.1" : "repeat") {
                                performControl { player.cycleRepeat() }
                            }
                        }
                        .buttonStyle(.borderless)
                        .padding(16)
                        .presentationCompactAdaptation(.popover)
                    }
                } else {
                    transportButton("随机播放", "shuffle", active: player.shuffle) { performControl { player.toggleShuffle() } }
                    transportButton(player.repeatMode.title, player.repeatMode == .one ? "repeat.1" : "repeat",
                                    active: player.repeatMode != .off) { performControl { player.cycleRepeat() } }
                }
                Spacer(minLength: 0)
                transport
                Spacer(minLength: 0)
                queueButton.frame(width: compact ? 44 : 88, alignment: .trailing)
            }
        }
        .frame(maxWidth: .infinity)
    }

    private var queueButton: some View {
        transportButton("播放队列", "list.bullet") { showQueue = true }
            .glassEffect(.regular.interactive(), in: .circle)
    }

    private var transport: some View {
        HStack(spacing: 3) {
            transportButton("上一首", "backward.fill") { performControl { player.previous() } }
            Button {
                performControl { player.toggle() }
            } label: {
                Group {
                    if player.loading { ProgressView() }
                    else { Image(systemName: player.playing ? "pause.fill" : "play.fill").font(.title2.weight(.semibold)) }
                }
                .foregroundStyle(player.coverTone.ink)
                .frame(width: 52, height: 52)
                .contentShape(.circle)
            }
            .buttonStyle(.plain)
            .glassEffect(.regular.interactive(), in: .circle)
            .accessibilityLabel(player.playing || player.loading ? "暂停" : "播放")
            .accessibilityIdentifier("now-playing-toggle")
            transportButton("下一首", "forward.fill") { performControl { player.advance() } }
        }
    }
}

private struct CurrentLyricPreview: View {
    @Environment(Player.self) private var player
    @Environment(ListenerModel.self) private var model

    var body: some View {
        if let lyrics = player.lyrics, !lyrics.lines.isEmpty {
            let index = lyrics.activeIndex(position: player.position, userOffset: model.settings?.lyricOffset ?? 0)
            let next = lyrics.lines.indices.first { $0 > (index ?? -1) && !lyrics.lines[$0].isBg }
            VStack(spacing: 3) {
                preview(index.map { lyrics.lines[$0].text }, current: true)
                preview(next.map { lyrics.lines[$0].text }, current: false)
            }
            .frame(maxWidth: .infinity)
        }
    }

    private func preview(_ line: String?, current: Bool) -> some View {
        Text(verbatim: line ?? "")
            .font(current ? .title3.weight(.semibold) : .body)
            .foregroundStyle(player.coverTone.ink.opacity(current ? 0.96 : 0.65))
            .lineLimit(1, reservesSpace: true)
            .multilineTextAlignment(.center)
            .minimumScaleFactor(0.82)
            .accessibilityHidden(line == nil)
    }
}

private struct CoverBackdrop: View {
    let image: UIImage?
    let tone: CoverTone

    var body: some View {
        GeometryReader { geometry in
            ZStack {
                LinearGradient(colors: [tone.shade(0.35), tone.shade(0.20)], startPoint: .top, endPoint: .bottom)
                if let image {
                    Image(uiImage: image).resizable().scaledToFill()
                        .frame(width: geometry.size.width + 110, height: geometry.size.height + 110)
                        .clipped()
                        .blur(radius: 60)
                        .opacity(0.60)
                        .mask(LinearGradient(stops: [
                            .init(color: .black, location: 0),
                            .init(color: .black.opacity(0.68), location: 0.49),
                            .init(color: .clear, location: 0.83),
                        ], startPoint: .top, endPoint: .bottom))
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
            .clipped()
        }
        .ignoresSafeArea()
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

struct CoverTone {
    let hue: CGFloat
    let saturation: CGFloat

    static let neutral = CoverTone(hue: 0, saturation: 0)
    var ink: Color { Color(hue: hue, saturation: min(0.095, saturation * 0.25), brightness: 1) }
    func shade(_ brightness: CGFloat) -> Color { Color(hue: hue, saturation: saturation, brightness: brightness) }

    static func sample(_ image: UIImage) -> Self {
        guard let source = image.cgImage else { return .neutral }
        let crop = CGRect(x: 0, y: source.height * 11 / 20, width: source.width, height: source.height * 9 / 20)
        guard let edge = source.cropping(to: crop),
              let context = CGContext(data: nil, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                      space: CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
              let data = context.data else { return .neutral }
        context.interpolationQuality = .high
        context.draw(edge, in: CGRect(x: 0, y: 0, width: 1, height: 1))
        let rgb = data.assumingMemoryBound(to: UInt8.self)
        let color = UIColor(red: CGFloat(rgb[0]) / 255, green: CGFloat(rgb[1]) / 255,
                            blue: CGFloat(rgb[2]) / 255, alpha: 1)
        var hue: CGFloat = 0
        var saturation: CGFloat = 0
        color.getHue(&hue, saturation: &saturation, brightness: nil, alpha: nil)
        return CoverTone(hue: hue, saturation: min(0.60, saturation * 0.9))
    }
}

private struct PlayerBackdrop: View {
    let track: Track
    let tone: CoverTone
    let animated: Bool

    var body: some View {
        GeometryReader { geometry in
            ZStack {
                LinearGradient(colors: [tone.shade(0.33), tone.shade(0.22)], startPoint: .top, endPoint: .bottom)
                AsyncImage(url: URL(string: track.cover)) { image in
                    let artwork = image.resizable().scaledToFill()
                        .frame(width: geometry.size.width + 190, height: geometry.size.height + 190)
                        .clipped()
                        .blur(radius: 54)
                        .saturation(1.5)
                        .drawingGroup()
                    Group {
                        if animated {
                            artwork.phaseAnimator([false, true]) { view, shifted in
                                view.scaleEffect(shifted ? 1.12 : 1.02)
                                    .offset(x: shifted ? 58 : -48, y: shifted ? -48 : 40)
                            } animation: { _ in .easeInOut(duration: 12) }
                        } else {
                            artwork
                        }
                    }
                    .opacity(0.75)
                } placeholder: { Color.clear }
                .id(track.cover)
                .transition(.opacity)
                Rectangle().fill(.linearGradient(
                    stops: [
                        .init(color: .black.opacity(0.34), location: 0),
                        .init(color: .black.opacity(0.40), location: 0.5),
                        .init(color: .black.opacity(0.76), location: 1),
                    ], startPoint: .top, endPoint: .bottom
                ))
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
            .clipped()
            .animation(animated ? .easeInOut(duration: 0.45) : nil, value: track.cover)
        }
        .ignoresSafeArea()
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

private struct SongMetadata: View {
    let track: Track
    var compact = false

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            TrackTitle(track: track, font: compact ? .headline : .title2.bold(), lineLimit: compact ? 1 : 2)
                .accessibilityAddTraits(.isHeader)
            Text(track.artist).font(compact ? .caption : .subheadline).foregroundStyle(.secondary).lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct Scrubber: View {
    @Environment(Player.self) private var player
    var onInteraction: (Bool) -> Void = { _ in }
    @State private var draft: Double?
    @GestureState private var touching = false

    var body: some View {
        let duration = player.duration
        let time = draft ?? player.position
        VStack(spacing: 0) {
            GeometryReader { geometry in
                let radius = 7.0
                let width = max(1, geometry.size.width - 2 * radius)
                let fraction = duration > 0 ? min(1, max(0, time / duration)) : 0
                ZStack(alignment: .leading) {
                    Capsule().fill(.primary.opacity(0.18)).frame(height: 4)
                        .overlay(alignment: .leading) {
                            Capsule().fill(.tint).frame(width: width * fraction)
                        }
                        .padding(.horizontal, radius)
                    Circle().fill(.primary)
                        .frame(width: 2 * radius, height: 2 * radius)
                        .offset(x: width * fraction)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                .contentShape(.rect)
                .gesture(DragGesture(minimumDistance: 0)
                    .updating($touching) { _, active, _ in active = true }
                    .onChanged { value in
                        if draft == nil { onInteraction(true) }
                        draft = duration * min(1, max(0, (value.location.x - radius) / width))
                    }
                    .onEnded { value in
                        commit(duration * min(1, max(0, (value.location.x - radius) / width)))
                    })
                .allowsHitTesting(duration > 0)
            }
            .frame(height: 44)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("播放进度")
            .accessibilityIdentifier("playback-scrubber")
            .accessibilityValue(clockText(time))
            .accessibilityAdjustableAction { direction in
                guard duration > 0 else { return }
                switch direction {
                case .increment: commit(min(duration, time + 5))
                case .decrement: commit(max(0, time - 5))
                @unknown default: break
                }
            }
            HStack {
                Text(clockText(time))
                Spacer()
                Text("−" + clockText(max(0, duration - time)))
            }
            .font(.caption.monospacedDigit())
            .foregroundStyle(.secondary)
        }
        .onChange(of: touching) { _, active in
            if !active && draft != nil {
                draft = nil
                onInteraction(false)
            }
        }
    }

    private func commit(_ value: Double) {
        player.seek(value)
        draft = nil
        onInteraction(false)
    }
}

private func clockText(_ time: Double) -> String {
    let seconds = Int(max(0, time))
    return "\(seconds / 60):" + String(format: "%02d", seconds % 60)
}

private func transportButton(_ label: String, _ icon: String, active: Bool = false, action: @escaping () -> Void) -> some View {
    Button(action: action) {
        Image(systemName: icon)
            .font(.body.weight(.semibold))
            .frame(minWidth: 44, minHeight: 44)
            .contentShape(.rect)
            .foregroundStyle(active ? AnyShapeStyle(.tint) : AnyShapeStyle(.primary))
            .background {
                if active { Capsule().fill(.primary.opacity(0.12)).frame(width: 32, height: 28) }
            }
    }
    .buttonStyle(.plain)
    .accessibilityLabel(label)
    .accessibilityAddTraits(active ? .isSelected : [])
}

private struct QueuePage: View {
    @Environment(Player.self) private var player
    @Environment(\.dismiss) private var dismiss
    @State private var editing = EditMode.inactive

    var body: some View {
        CatalogNavigation {
            List {
                ForEach(player.queue) { track in
                    Button {
                        if player.current?.mid == track.mid { player.toggle() }
                        else { player.play(track) }
                    } label: {
                        HStack(spacing: 9) {
                            Cover(url: track.cover, radius: 7).frame(width: 34, height: 34)
                            VStack(alignment: .leading, spacing: 1) {
                                TrackTitle(track: track, font: .subheadline.weight(.medium), lineLimit: 1)
                                Text(track.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                            }
                            Spacer(minLength: 6)
                            if player.current?.mid == track.mid {
                                Image(systemName: "waveform").foregroundStyle(.tint)
                            }
                        }
                        .frame(minHeight: 44)
                        .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .listRowInsets(.init(top: 2, leading: 16, bottom: 2, trailing: 16))
                    .contextMenu { TrackCatalogActions(track: track) }
                    .swipeActions {
                        if player.current?.mid != track.mid {
                            Button("移除", systemImage: "minus.circle", role: .destructive) { player.remove(track) }
                        }
                    }
                }
                .onMove(perform: player.move)
            }
            .navigationTitle("播放队列")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button("关闭", systemImage: "xmark") { dismiss() } }
                ToolbarItem(placement: .topBarTrailing) {
                    Button(editing.isEditing ? "完成" : "编辑") {
                        withAnimation { editing = editing.isEditing ? .inactive : .active }
                    }
                }
            }
            .environment(\.editMode, $editing)
        }
    }
}

private let lyricFollowMotion = Animation.smooth(duration: 0.36)

private struct LyricRowBounds: PreferenceKey {
    static var defaultValue: [Int: Anchor<CGRect>] { [:] }

    static func reduce(value: inout [Int: Anchor<CGRect>], nextValue: () -> [Int: Anchor<CGRect>]) {
        value.merge(nextValue(), uniquingKeysWith: { _, next in next })
    }
}

private enum LyricAnchor: Hashable {
    case line(Int)
    case interlude(Int)
}

private struct LyricPanel: View {
    let track: Track
    let ink: Color
    let browsingChanged: (Bool) -> Void
    @Environment(Player.self) private var player
    @Environment(ListenerModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ScaledMetric(relativeTo: .title2) private var lyricSize = 35.0
    @GestureState private var finger: CGPoint?
    @State private var browsing = false
    @State private var touching = false
    @State private var revealedIndex: Int?
    @State private var followAt: Int?
    @State private var readyToReturn = false
    @State private var followDeadlineReached = false
    @State private var timer: Task<Void, Never>?
    @State private var interludeRestart: (nextLine: Int, time: Double)?

    private var lines: [LyricLine] { player.lyrics?.lines ?? [] }
    private var offset: Double { Double((model.settings?.lyricOffset ?? 0) + (player.lyrics?.offset ?? 0)) }
    private var active: Int? {
        player.lyrics?.activeIndex(position: player.position, userOffset: model.settings?.lyricOffset ?? 0)
    }
    private var interlude: LyricInterlude? {
        let time = (player.position * 1000 + offset).rounded()
        return player.lyrics?.interludes.first { Double($0.start) <= time && time < Double($0.end) }
    }
    private var interludePerformance: InterludePerformance? {
        guard let interlude else { return nil }
        let restart = interludeRestart?.nextLine == interlude.nextLine ? interludeRestart?.time : nil
        return InterludePerformance(gap: interlude, restartAt: restart)
    }
    private func focus(_ performance: InterludePerformance?) -> LyricAnchor? {
        if let performance { return .interlude(performance.nextLine) }
        return active.map(LyricAnchor.line)
    }
    private func restartInterlude() {
        interludeRestart = interlude.map { ($0.nextLine, player.position * 1000 + offset) }
    }

    var body: some View {
        Group {
            if player.lyricsLoading {
                ProgressView("正在加载歌词")
            } else if let error = player.lyricError {
                LoadError(message: error) { player.fetchLyrics(for: track) }
            } else if lines.isEmpty {
                ContentUnavailableView("暂无同步歌词", systemImage: "music.note")
            } else {
                scroller
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onDisappear { timer?.cancel() }
    }

    private var scroller: some View {
        GeometryReader { geometry in
        ScrollViewReader { proxy in
            let performance = interludePerformance
            let target = focus(performance)
            let textWidth = max(0, geometry.size.width - 14)
            ScrollView {
                lyricRows(performance, focused: target, textWidth: textWidth, proxy: proxy)
            }
            .contentMargins(.top, geometry.size.height * 0.36, for: .scrollContent)
            .contentMargins(.bottom, geometry.size.height * 0.60, for: .scrollContent)
            .frame(width: geometry.size.width)
            .scrollIndicators(.automatic)
            .accessibilityIdentifier("lyric-scroll")
            .overlayPreferenceValue(LyricRowBounds.self) { anchors in
                GeometryReader { hitTest in
                    let hit = touching ? finger.flatMap { point in
                        anchors.first { hitTest[$0.value].contains(point) }?.key
                    } : nil
                    Color.clear.onChange(of: hit, initial: true) { _, index in
                        if touching && finger != nil { revealedIndex = index }
                    }
                }
                .allowsHitTesting(false)
                .accessibilityHidden(true)
            }
            .coordinateSpace(.named("lyric-scroll"))
            .simultaneousGesture(DragGesture(minimumDistance: 0, coordinateSpace: .named("lyric-scroll"))
                .updating($finger) { value, location, _ in location = value.location })
            .mask {
                VStack(spacing: 0) {
                    LinearGradient(colors: [.clear, .black], startPoint: .top, endPoint: .bottom)
                        .frame(height: 32)
                    Rectangle()
                    LinearGradient(colors: [.black, .clear], startPoint: .top, endPoint: .bottom)
                        .frame(height: 20)
                }
            }
            .defaultScrollAnchor(.top)
            .onAppear { restartInterlude() }
            .onChange(of: interlude?.nextLine) { _, next in
                if interludeRestart?.nextLine != next { interludeRestart = nil }
            }
            .onScrollPhaseChange { old, new in
                if new == .interacting {
                    timer?.cancel()
                    touching = true
                    revealedIndex = nil
                    readyToReturn = false
                    followDeadlineReached = false
                    beginBrowsing()
                } else if old == .interacting {
                    touching = false
                    revealedIndex = nil
                    scheduleReturn()
                }
            }
            .onChange(of: target, initial: true) { _, next in
                if !browsing, let next { center(next, proxy) }
                else { returnIfReady(proxy) }
            }
            .onChange(of: readyToReturn) { _, _ in returnIfReady(proxy) }
            .onChange(of: followDeadlineReached) { _, _ in returnIfReady(proxy) }
            .onChange(of: player.seekRevision) { _, _ in
                resetBrowsing()
                restartInterlude()
                if let target = focus(interludePerformance) { center(target, proxy) }
            }
            .onChange(of: offset) { _, _ in
                restartInterlude()
            }
            .onChange(of: player.playing) { _, _ in
                if browsing && !touching { scheduleReturn() }
            }
        }
        }
    }

    private func lyricRows(_ performance: InterludePerformance?, focused: LyricAnchor?, textWidth: CGFloat,
                           proxy: ScrollViewProxy) -> some View {
        LazyVStack(alignment: .leading, spacing: 22) {
            ForEach(lines.indices, id: \.self) { index in
                if let performance, performance.nextLine == index {
                    InterludeDots(performance: performance, offset: offset, size: lyricSize, ink: ink)
                        .id(LyricAnchor.interlude(index))
                }
                lyricRow(lines[index], sustained: player.lyrics?.sustainedWords[index] ?? [], focused: focused == .line(index),
                         revealed: index == revealedIndex, textWidth: textWidth) { center(.line(index), proxy) }
                    .id(LyricAnchor.line(index))
                    .anchorPreference(key: LyricRowBounds.self, value: .bounds) { [index: $0] }
            }
            if let warning = player.lyrics?.warnings.first {
                Text(warning).font(.footnote).foregroundStyle(.secondary)
            }
        }
        .padding(.leading, 4)
        .padding(.trailing, 10)
    }

    private func lyricRow(_ line: LyricLine, sustained: Set<Int>, focused: Bool, revealed: Bool,
                          textWidth: CGFloat, selected: @escaping () -> Void) -> some View {
        let scale = Double(model.settings?.lyrics.fontScale ?? 100) / 100
        let fontSize = lyricSize * scale * (line.isBg ? 0.73 : 1)
        let spacing = max(0, (model.settings?.lyrics.lineHeight ?? 1.5) - 1) * 10
        let hasText: (LyricWord) -> Bool = { !$0.word.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
        let firstWord = line.words.firstIndex(where: hasText) ?? 0
        let lastWord = line.words.lastIndex(where: hasText) ?? -1
        let timed = line.words.indices.dropFirst(firstWord).prefix(while: { $0 <= lastWord }).reduce(Text(verbatim: "")) { text, index in
            let word = line.words[index]
            var value = word.word[...]
            if index == firstWord { value = value.drop(while: \.isWhitespace) }
            if index == lastWord { while value.last?.isWhitespace == true { value.removeLast() } }
            let piece = Text(verbatim: String(value))
                .customAttribute(LyricTiming(index: index, word: word, canEmphasize: sustained.contains(index)))
            return Text("\(text)\(piece)")
        }
        let translation = line.translatedLyric.trimmingCharacters(in: .whitespacesAndNewlines)
        let romanization = line.romanLyric.trimmingCharacters(in: .whitespacesAndNewlines)
        let sweeping = focused && (player.lyrics?.wordTimed ?? (line.words.count > 1))
        let blurred = !focused && !revealed && !browsing && !reduceMotion
        return Button {
            if blurred {
                beginBrowsing()
                scheduleReturn()
                return
            }
            let previous = active
            resetBrowsing()
            player.seek((Double(line.startTime) - offset) / 1000)
            if active == previous { selected() }
        } label: {
            VStack(alignment: .leading, spacing: 4) {
                TimelineView(.animation(paused: !sweeping || !player.playing)) { _ in
                    timed.font(.system(size: fontSize, weight: .bold))
                        .lineSpacing(spacing)
                        .textRenderer(LyricSweep(time: sweeping ? player.lyricPosition * 1000 + offset : nil,
                                                 base: revealed ? 0.9 : 0.41, fontSize: fontSize, motionEnabled: !reduceMotion))
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(width: textWidth, alignment: .leading)
                }
                if model.settings?.lyrics.showTranslation != false, !translation.isEmpty {
                    Text(verbatim: translation).font(.callout.weight(.semibold)).foregroundStyle(ink.opacity(0.76))
                }
                if model.settings?.lyrics.showRomanization == true, !romanization.isEmpty {
                    Text(verbatim: romanization).font(.callout).foregroundStyle(ink.opacity(0.68))
                }
            }
            .foregroundStyle(ink)
            .lineLimit(nil)
            .multilineTextAlignment(.leading)
            // Keep the row's measured bounds and touch target stable while its text moves.
            .scaleEffect(reduceMotion || focused ? 1 : 0.966, anchor: .topLeading)
            .offset(y: focused && !reduceMotion ? -3.5 : 0)
            .animation(reduceMotion ? nil : .spring(duration: 0.42, bounce: 0.12), value: focused)
            .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityHint(blurred ? "显示清晰歌词和播放控制" : "跳到这句的播放进度")
        .opacity(focused || revealed ? 1 : 0.43)
        .blur(radius: blurred ? 1.6 : 0)
    }

    private func beginBrowsing() {
        withAnimation(reduceMotion ? nil : .easeOut(duration: 0.22)) { browsing = true }
        browsingChanged(true)
    }

    private func resetBrowsing() {
        timer?.cancel()
        browsing = false
        touching = false
        revealedIndex = nil
        readyToReturn = false
        followDeadlineReached = false
        browsingChanged(false)
    }

    private func scheduleReturn() {
        timer?.cancel()
        readyToReturn = false
        followDeadlineReached = false
        guard player.playing else { return }
        let released = ContinuousClock.now
        timer = Task {
            do {
                try await Task.sleep(until: released.advanced(by: .milliseconds(2500)), clock: .continuous)
                followAt = lines.indices.first { $0 > (active ?? -1) && !lines[$0].isBg }
                readyToReturn = true
                try await Task.sleep(until: released.advanced(by: .seconds(5)), clock: .continuous)
                followDeadlineReached = true
            } catch {}
        }
    }

    private func returnIfReady(_ proxy: ScrollViewProxy) {
        guard browsing, !touching, readyToReturn, player.playing else { return }
        let target = focus(interludePerformance) ?? lines.indices.first { !lines[$0].isBg }.map(LyricAnchor.line)
        let arrivedAtNextLine = active.map { current in followAt.map { current >= $0 } ?? false } ?? false
        guard let target, followDeadlineReached || arrivedAtNextLine else { return }
        center(target, proxy, resumingFollow: true)
    }

    private func center(_ anchor: LyricAnchor, _ proxy: ScrollViewProxy, resumingFollow: Bool = false) {
        withAnimation(reduceMotion ? nil : lyricFollowMotion) {
            if resumingFollow { resetBrowsing() }
            proxy.scrollTo(anchor, anchor: UnitPoint(x: 0.5, y: 0.38))
        }
    }

}

// Mirrors the playback-time stages of AMLL's InterludeDotsBase on macOS.
private struct InterludePerformance {
    struct Frame {
        let opacity: Double
        let scale: Double
        let dots: [Double]
    }

    private static let trailing = 750.0
    private static let exitDuration = 1000.0
    private static let enterFade = CubicEase(x1: 0.59, y1: 0.02, x2: 0.07, y2: 1)
    private static let light = CubicEase(x1: 0.56, y1: 0.01, x2: 0.45, y2: 1)
    private static let exitGrow = CubicEase(x1: 0.14, y1: 0.06, x2: 0.25, y2: 1)
    private static let exitShrink = CubicEase(x1: 0.29, y1: 0.03, x2: 1, y2: 0.38)
    private static let exitFade = CubicEase(x1: 0.43, y1: 0.08, x2: 0.83, y2: 0.31)

    let nextLine: Int
    private let start: Double
    private let end: Double
    private let bodyDuration: Double
    private let period: Double
    private let segment: Double
    private let thirdDuration: Double
    private let thirdTarget: Double

    init?(gap: LyricInterlude, restartAt: Double?) {
        let delay = restartAt == nil && gap.nextLine == 0 ? 0.0 : 500.0
        let start = (restartAt ?? Double(gap.start)) + delay
        let end = Double(gap.end)
        let body = end - start - Self.exitDuration
        guard body >= 910 else { return nil }
        nextLine = gap.nextLine
        self.start = start
        self.end = end
        bodyDuration = body
        period = body / max(1, floor(body / 4000))
        segment = ((body + Self.trailing) / 3).rounded()
        thirdDuration = body - segment * 2
        thirdTarget = body < 3000 ? 1 : thirdDuration / segment
    }

    func frame(at time: Double, reduceMotion: Bool) -> Frame {
        let elapsed = time - start
        guard elapsed >= 0, time < end else { return Frame(opacity: 0, scale: 1, dots: [0, 0, 0]) }
        var opacity = Self.enterFade(elapsed / 180)
        let scale: Double
        let fractions: [Double]
        if elapsed >= bodyDuration {
            let closing = elapsed - bodyDuration
            opacity *= 1 - Self.exitFade((closing - Self.trailing) / 250)
            scale = closing < Self.trailing
                ? 1 + 0.25 * Self.exitGrow(closing / Self.trailing)
                : 1.25 - 0.85 * Self.exitShrink((closing - Self.trailing) / 250)
            fractions = [1, 1, thirdTarget + (1 - thirdTarget) * min(1, closing / Self.trailing)]
        } else if bodyDuration < 3000 {
            scale = 1
            fractions = [1, 1, 1]
        } else {
            let phase = elapsed.truncatingRemainder(dividingBy: period) / period
            let angle = 4 * Double.pi * phase
            let s = sin(angle), c = cos(angle)
            let breath = phase - 0.084 * s + 0.008 * (1 - c) + 0.0046 * s * (c - s)
            scale = 1 + 0.5 * min(breath, 1 - breath)
            fractions = [Self.light(elapsed / segment), Self.light((elapsed - segment) / segment),
                         Self.light((elapsed - 2 * segment) / thirdDuration) * thirdTarget]
        }
        let dots = fractions.enumerated().map { index, fraction in
            let enter = min(1, max(0, (elapsed - Double(index) * 80) / 750))
            return (0.2 + 0.7 * fraction) * enter * enter
        }
        return Frame(opacity: opacity, scale: reduceMotion ? 1 : scale, dots: dots)
    }

    private struct CubicEase {
        let x1, y1, x2, y2: Double

        func callAsFunction(_ x: Double) -> Double {
            guard x > 0, x < 1 else { return min(1, max(0, x)) }
            func coordinate(_ t: Double, _ a: Double, _ b: Double) -> Double {
                let u = 1 - t
                return 3 * u * u * t * a + 3 * u * t * t * b + t * t * t
            }
            var low = 0.0, high = 1.0
            for _ in 0..<14 {
                let t = (low + high) / 2
                if coordinate(t, x1, x2) < x { low = t } else { high = t }
            }
            return coordinate((low + high) / 2, y1, y2)
        }
    }
}

private struct InterludeDots: View {
    let performance: InterludePerformance
    let offset: Double
    let size: CGFloat
    let ink: Color
    @Environment(Player.self) private var player
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(paused: !player.playing)) { _ in
            let frame = performance.frame(at: player.lyricPosition * 1000 + offset, reduceMotion: reduceMotion)
            HStack(spacing: size * 0.20) {
                ForEach(0..<3) { index in
                    Circle().fill(ink)
                        .frame(width: size * 0.30, height: size * 0.30)
                        .opacity(frame.dots[index])
                }
            }
            .scaleEffect(frame.scale, anchor: .leading)
            .opacity(frame.opacity)
            .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("间奏")
        .accessibilityIdentifier("lyric-interlude-dots")
    }

}

private struct LyricTiming: TextAttribute {
    let index: Int
    let start, end: UInt64
    let canEmphasize: Bool

    init(index: Int, word: LyricWord, canEmphasize: Bool) {
        self.index = index
        start = word.startTime
        end = word.endTime
        self.canEmphasize = canEmphasize
    }
}

private struct LyricSweep: TextRenderer {
    let time: Double?
    let base: Double
    let fontSize: CGFloat
    let motionEnabled: Bool

    // Reserve room for vertical motion without letting the drawn text wrap wider than its measured layout.
    var displayPadding: EdgeInsets { EdgeInsets(top: 16, leading: 0, bottom: 12, trailing: 0) }

    func draw(layout: Text.Layout, in context: inout GraphicsContext) {
        guard let time else {
            for row in layout { for run in row { context.draw(run) } }
            return
        }
        var widths: [Int: CGFloat] = [:]
        var consumed: [Int: CGFloat] = [:]
        for row in layout {
            for run in row {
                if let timing = run[LyricTiming.self] {
                    widths[timing.index, default: 0] += run.typographicBounds.width
                }
            }
        }

        for row in layout {
            for run in row {
                guard let timing = run[LyricTiming.self] else {
                    context.draw(run)
                    continue
                }
                let bounds = run.typographicBounds.rect
                let start = Double(timing.start)
                let duration = max(1, Double(timing.end) - start)
                let elapsed = time - start
                let progress = min(1, max(0, elapsed / duration))
                let previousWidth = consumed[timing.index, default: 0]
                consumed[timing.index] = previousWidth + bounds.width
                let distance = (widths[timing.index] ?? bounds.width) * progress - previousWidth
                let direction: CGFloat = run.layoutDirection == .rightToLeft ? -1 : 1
                var word = context
                var emphasis = 0.0
                if motionEnabled && elapsed > 0 {
                    let rise = min(1, elapsed / max(850, duration))
                    let easedRise = rise * (2 - rise)
                    if timing.canEmphasize && elapsed > 750 && elapsed < duration {
                        let swell = (elapsed - 750) / (duration - 750)
                        emphasis = pow(sin(swell * .pi), 2)
                    }
                    let lift = fontSize * (0.047 * easedRise + 0.020 * emphasis)
                    let pivot = direction > 0 ? bounds.minX : bounds.maxX
                    word.translateBy(x: pivot, y: bounds.maxY - lift)
                    word.scaleBy(x: 1 + emphasis * min(0.02, 2 / max(1, bounds.width)), y: 1 + emphasis * 0.03)
                    word.translateBy(x: -pivot, y: -bounds.maxY)
                }
                if progress >= 1 || distance >= bounds.width {
                    drawLit(run, in: word, halo: emphasis * 0.5)
                    continue
                }
                var dimmed = word
                dimmed.opacity *= base
                dimmed.draw(run)
                guard distance > 0 else { continue }

                // Mask a laid-out run, so the sweep follows the glyphs across wrapped lines.
                let edge = direction > 0 ? bounds.minX + distance : bounds.maxX - distance
                let feather = min(8, max(2, bounds.width * 0.12))
                var lit = word
                lit.clipToLayer { mask in
                    mask.fill(Path(bounds.insetBy(dx: -10, dy: -10)), with: .linearGradient(
                        Gradient(colors: [.white, .clear]),
                        startPoint: CGPoint(x: edge - feather * direction, y: bounds.midY),
                        endPoint: CGPoint(x: edge + feather * direction, y: bounds.midY)
                    ))
                }
                drawLit(run, in: lit, halo: max(sin(progress * .pi) * 0.64, emphasis * 0.5))
            }
        }
    }

    private func drawLit(_ run: Text.Layout.Run, in context: GraphicsContext, halo strength: Double) {
        if motionEnabled && strength > 0 {
            var halo = context
            halo.opacity *= strength
            halo.addFilter(.blur(radius: 3.5))
            halo.draw(run)
        }
        context.draw(run)
    }
}
