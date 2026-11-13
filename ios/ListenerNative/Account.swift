import SwiftUI
import Photos
import SafariServices

struct NativeSettings: Codable, Equatable {
    var audioQuality: Quality
    var lyricOffset: Int64
    var animatedBackground: Bool
    var lyrics: LyricsSettings

    enum Quality: String, CaseIterable, Codable {
        case auto, standard, high, lossless, hires
        var title: String {
            switch self {
            case .auto: "自动"
            case .standard: "标准"
            case .high: "高品质"
            case .lossless: "无损"
            case .hires: "Hi-Res"
            }
        }
    }

    struct LyricsSettings: Codable, Equatable {
        var fontScale: UInt16
        var lineHeight: Double
        var showTranslation: Bool
        var showRomanization: Bool
    }
}

struct AccountGate: View {
    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player
    @State private var quick = false
    @State private var qr = false
    @State private var sms = false
    @State private var error: String?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 22) {
                    Image(systemName: "waveform")
                        .font(.system(size: 46, weight: .medium))
                        .foregroundStyle(.tint)
                        .frame(width: 94, height: 94)
                        .background(.tint.opacity(0.10), in: .rect(cornerRadius: 24))
                        .accessibilityHidden(true)
                    VStack(spacing: 8) {
                        Text("Listener").font(.largeTitle.bold())
                        Text("连接 QQ 音乐").font(.title3.weight(.semibold))
                    }
                    if let message = error ?? model.sessionError {
                        Text(message)
                            .font(.subheadline).foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                    }
                    VStack(spacing: 10) {
                        Button {
                            Task { await authorize() }
                        } label: {
                            if quick { ProgressView().frame(maxWidth: .infinity) }
                            else { Label("用 QQ 登录", systemImage: "person.crop.circle").frame(maxWidth: .infinity) }
                        }
                        .buttonStyle(.glassProminent)
                        .controlSize(.large)
                        .disabled(quick)
                        Button {
                            qr = true
                        } label: {
                            Label("二维码登录", systemImage: "qrcode").frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.glass)
                        .controlSize(.large)
                        .disabled(quick)
                        Button("手机号登录", systemImage: "phone") { sms = true }
                            .frame(minHeight: 44)
                            .disabled(quick)
                        if model.sessionError != nil {
                            Button("重试账户连接") { Task { await model.restore(player: player) } }
                                .frame(minHeight: 44)
                        }
                    }
                }
                .frame(maxWidth: 420)
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 32)
                .padding(.vertical, 46)
            }
            .safeAreaPadding(.top, 26)
            .sheet(isPresented: $qr) { QRLogin() }
            .sheet(isPresented: $sms) { SMSLogin() }
        }
    }

    private func authorize() async {
        quick = true
        error = nil
        defer { quick = false }
        do {
            if let profile = try await Core.authorizeQQ() { await model.connect(profile, player: player) }
        } catch { self.error = error.localizedDescription }
    }
}

private struct SMSLogin: View {
    struct Reply: Decodable { let status: String; let generation: UInt64 }
    struct Challenge: Identifiable { let url: URL; var id: URL { url } }

    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player
    @Environment(\.dismiss) private var dismiss
    @State private var phone = ""
    @State private var code = ""
    @State private var reply: Reply?
    @State private var challenge: Challenge?
    @State private var error: String?
    @State private var busy = false
    @State private var finished = false

    var body: some View {
        NavigationStack {
            Form {
                Section("中国大陆手机号") {
                    TextField("11 位手机号", text: $phone)
                        .keyboardType(.phonePad)
                        .textContentType(.telephoneNumber)
                        .disabled(busy)
                    Button("发送验证码") { Task { await send() } }
                        .disabled(!digits(phone, count: 11) || busy)
                }
                if let reply, reply.status == "challenge" {
                    Section {
                        Button("打开 QQ 音乐安全验证", systemImage: "checkmark.shield") {
                            Task { await openChallenge(reply.generation) }
                        }.disabled(busy)
                    } footer: { Text("验证完成后返回这里重新发送验证码。") }
                } else if let reply, reply.status == "sent" {
                    Section("短信验证码") {
                        TextField("6 位验证码", text: $code)
                            .keyboardType(.numberPad)
                            .textContentType(.oneTimeCode)
                        Button("登录") { Task { await verify(reply.generation) } }
                            .disabled(!digits(code, count: 6) || busy)
                    }
                }
                if busy { ProgressView() }
                if let error { Text(error).foregroundStyle(.secondary) }
            }
            .onChange(of: phone) { _, _ in reply = nil; code = ""; error = nil }
            .navigationTitle("手机号登录")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button("关闭", systemImage: "xmark") { dismiss() } }
            }
        }
        .sheet(item: $challenge) { SafariChallenge(url: $0.url).ignoresSafeArea() }
        .onDisappear { if !finished && challenge == nil { Task { try? await Core.perform("cancelLogin") } } }
    }

    private func digits(_ value: String, count: Int) -> Bool {
        value.utf8.count == count && value.utf8.allSatisfy { (48...57).contains($0) }
    }

    private func send() async {
        busy = true
        error = nil
        defer { busy = false }
        do { reply = try await Core.call("sendSms", ["phone": phone]) }
        catch { self.error = error.localizedDescription }
    }

    private func openChallenge(_ generation: UInt64) async {
        busy = true
        error = nil
        defer { busy = false }
        do {
            let url: URL = try await Core.call("smsChallenge", ["generation": generation])
            challenge = Challenge(url: url)
        } catch { self.error = error.localizedDescription }
    }

    private func verify(_ generation: UInt64) async {
        busy = true
        error = nil
        defer { busy = false }
        do {
            let profile: Profile = try await Core.call("verifySms", ["generation": generation, "code": code])
            finished = true
            await model.connect(profile, player: player)
            dismiss()
        } catch { self.error = error.localizedDescription }
    }
}

private struct SafariChallenge: UIViewControllerRepresentable {
    let url: URL
    func makeUIViewController(context: Context) -> SFSafariViewController { SFSafariViewController(url: url) }
    func updateUIViewController(_ controller: SFSafariViewController, context: Context) {}
}

private struct QRLogin: View {
    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player
    @Environment(\.dismiss) private var dismiss
    @State private var kind = "qq"
    @State private var code: QR?
    @State private var error: String?
    @State private var hint: String?
    @State private var refresh = 0
    @State private var finished = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 18) {
                    Picker("扫码方式", selection: $kind) {
                        Text("QQ").tag("qq")
                        Text("微信").tag("wx")
                        Text("QQ 音乐").tag("mobile")
                    }
                    .pickerStyle(.segmented)
                    if let data = code?.data, let image = UIImage(data: data) {
                        Image(uiImage: image).interpolation(.none).resizable().scaledToFit()
                            .frame(width: 196, height: 196)
                            .padding(12)
                            .background(.white, in: .rect(cornerRadius: 12))
                            .accessibilityLabel("登录二维码")
                    } else if error == nil {
                        ProgressView().frame(minHeight: 196)
                    }
                    if let error {
                        Text(error).foregroundStyle(.secondary).multilineTextAlignment(.center)
                    } else {
                        Text(hint ?? "在对应 App 扫码并确认登录。")
                            .foregroundStyle(.secondary).multilineTextAlignment(.center)
                    }
                    if code != nil || error != nil {
                        Button("刷新二维码", systemImage: "arrow.clockwise") { refresh += 1 }
                            .buttonStyle(.glass)
                    }
                    if let data = code?.data {
                        Button("保存到相册", systemImage: "square.and.arrow.down") { Task { await save(data) } }
                            .buttonStyle(.glass)
                    }
                }
                .frame(maxWidth: 360)
                .frame(maxWidth: .infinity)
                .padding(24)
            }
            .navigationTitle("二维码登录")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button("关闭", systemImage: "xmark") { dismiss() } }
            }
        }
        .task(id: "\(kind):\(refresh)") { await poll() }
        .onDisappear {
            if !finished { Task { try? await Core.perform("cancelLogin") } }
        }
    }

    private func poll() async {
        let selection = kind
        let generation = refresh
        code = nil
        error = nil
        hint = nil
        do {
            let value: QR = try await Core.call("qr", ["kind": selection])
            try Task.checkCancellation()
            guard selection == kind, generation == refresh else { return }
            code = value
            while !Task.isCancelled {
                try await Task.sleep(for: .seconds(1.5))
                let status: LoginPoll = try await Core.call("poll", ["id": value.id])
                try Task.checkCancellation()
                guard selection == kind, generation == refresh else { return }
                switch status.status {
                case "complete":
                    guard let profile = status.profile else {
                        throw CoreError(code: "PROFILE_MISSING", message: "QQ 音乐未返回账户资料。")
                    }
                    finished = true
                    await model.connect(profile, player: player)
                    dismiss()
                    return
                case "scanned": hint = "已扫码，请在授权 App 确认。"
                case "waiting": break
                case "expired", "closed", "refused":
                    code = nil
                    error = status.status == "refused" ? "登录已拒绝。" : "二维码已失效，请刷新。"
                    return
                default: throw CoreError(code: "LOGIN_STATUS", message: "无法识别 QQ 音乐的登录状态。")
                }
            }
        } catch is CancellationError {}
        catch { if !Task.isCancelled { self.error = error.localizedDescription } }
    }

    private func save(_ data: Data) async {
        let status = await PHPhotoLibrary.requestAuthorization(for: .addOnly)
        guard status == .authorized || status == .limited else {
            error = "请在系统设置中允许 Listener 添加照片。"
            return
        }
        do {
            try await PHPhotoLibrary.shared().performChanges {
                PHAssetCreationRequest.forAsset().addResource(with: .photo, data: data, options: nil)
            }
            hint = "已存入相册，可从扫码界面选取。"
        } catch { self.error = "保存二维码失败，请重试。" }
    }
}

struct SettingsPage: View {
    @Environment(ListenerModel.self) private var model
    @Environment(Player.self) private var player
    @Environment(\.dismiss) private var dismiss
    @AppStorage("listener.native.appearance") private var appearance = "system"
    @AppStorage("listener.native.qqAutoMix") private var qqAutoMix = true
    @State private var settings: NativeSettings?
    @State private var saveError: String?
    @State private var confirmingLogout = false
    @State private var saving = false

    var body: some View {
        NavigationStack {
            Form {
                if let profile = model.profile {
                    Section("QQ 音乐") {
                        HStack(spacing: 12) {
                            Cover(url: profile.avatar, radius: 20).frame(width: 40, height: 40)
                            Text(profile.nickname).lineLimit(2)
                        }
                        Button("退出登录", role: .destructive) { confirmingLogout = true }
                    }
                }
                Section("外观") {
                    Picker("主题", selection: $appearance) {
                        Text("跟随系统").tag("system")
                        Text("浅色").tag("light")
                        Text("深色").tag("dark")
                    }
                }
                if settings != nil {
                    settingsForm
                } else if saveError == nil {
                    ProgressView("正在读取设置")
                }
                if let saveError { Text(saveError).foregroundStyle(.secondary) }
            }
            .navigationTitle("设置")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("关闭", systemImage: "xmark") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("保存") { Task { await save() } }
                        .disabled(saving || settings == nil || settings == model.settings)
                }
            }
            .confirmationDialog("退出 QQ 音乐", isPresented: $confirmingLogout, titleVisibility: .visible) {
                Button("退出登录", role: .destructive) { Task { await logout() } }
            }
        }
        .task {
            do { settings = try await Core.call("settings") }
            catch { saveError = error.localizedDescription }
        }
    }

    private var settingsForm: some View {
        let values = Binding(get: { settings! }, set: { settings = $0 })
        return Group {
            Section("播放") {
                Picker("音质", selection: values.audioQuality) {
                    ForEach(NativeSettings.Quality.allCases, id: \.self) { Text($0.title).tag($0) }
                }
                Toggle("动态封面背景", isOn: values.animatedBackground)
                Toggle("QQ 自动衔接", isOn: $qqAutoMix)
                if qqAutoMix, let notice = player.mixNotice {
                    Text(notice).font(.footnote).foregroundStyle(.secondary)
                }
            }
            Section("歌词") {
                Toggle("译文", isOn: values.lyrics.showTranslation)
                Toggle("音译", isOn: values.lyrics.showRomanization)
                Stepper("字号 \(values.wrappedValue.lyrics.fontScale)%", value: values.lyrics.fontScale, in: 70...160, step: 5)
                Stepper("行距 \(values.wrappedValue.lyrics.lineHeight.formatted(.number.precision(.fractionLength(1))))",
                        value: values.lyrics.lineHeight, in: 1...2.2, step: 0.1)
                Stepper("时间偏移 \(values.wrappedValue.lyricOffset) ms", value: values.lyricOffset, in: -5000...5000, step: 100)
            }
        }
    }

    private func save() async {
        guard let settings else { return }
        saving = true
        saveError = nil
        defer { saving = false }
        do {
            try await Core.perform("saveSettings", ["settings": settings])
            model.settings = settings
            dismiss()
        } catch { saveError = error.localizedDescription }
    }

    private func logout() async {
        do {
            await player.flush()
            try await Core.perform("logout")
            await model.connect(nil, player: player)
            dismiss()
        } catch { saveError = error.localizedDescription }
    }
}
