import Foundation
import AppKit
import ApplicationServices
import CoreGraphics
import CoreAudio
import AVFoundation
import Vision
import EventKit
import ScreenCaptureKit

// ============================================================================
// backtext-macos-sensor — Native Apple Silicon macOS Sensor Binary
// Frameworks: ScreenCaptureKit, Vision OCR, AXUIElement, CoreAudio, AVFoundation, EventKit
// ============================================================================

struct JSONOut {
    static func emit(_ obj: Any) {
        if let data = try? JSONSerialization.data(withJSONObject: obj, options: []),
           let str = String(data: data, encoding: .utf8) {
            print(str)
            fflush(stdout)
        } else {
            print("{}")
            fflush(stdout)
        }
    }
}

func jsonNullable(_ value: String?) -> Any {
    if let v = value {
        return v
    }
    return NSNull()
}

func deviceHasInputChannels(_ deviceID: AudioDeviceID) -> Bool {
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyStreamConfiguration,
        mScope: kAudioDevicePropertyScopeInput,
        mElement: kAudioObjectPropertyElementMain
    )
    var dataSize: UInt32 = 0
    let sizeStatus = AudioObjectGetPropertyDataSize(deviceID, &addr, 0, nil, &dataSize)
    guard sizeStatus == noErr, dataSize >= UInt32(MemoryLayout<AudioBufferList>.size) else {
        return false
    }
    var raw = Data(count: Int(dataSize))
    let readStatus: OSStatus = raw.withUnsafeMutableBytes { buf -> OSStatus in
        guard let base = buf.baseAddress else { return -1 }
        return AudioObjectGetPropertyData(deviceID, &addr, 0, nil, &dataSize, base)
    }
    guard readStatus == noErr else { return false }
    return raw.withUnsafeBytes { buf -> Bool in
        guard let ptr = buf.bindMemory(to: AudioBufferList.self).baseAddress else { return false }
        let list = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: ptr))
        for audioBuf in list {
            if audioBuf.mNumberChannels > 0 {
                return true
            }
        }
        return false
    }
}

func isDeviceRunningSomewhere(_ deviceID: AudioDeviceID) -> Bool {
    var isRunning: UInt32 = 0
    var runSize = UInt32(MemoryLayout<UInt32>.size)
    var runAddr = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyDeviceIsRunningSomewhere,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    let runStatus = AudioObjectGetPropertyData(deviceID, &runAddr, 0, nil, &runSize, &isRunning)
    return runStatus == noErr && isRunning != 0
}

func isDefaultMicRunning() -> Bool {
    var deviceID = AudioDeviceID(0)
    var size = UInt32(MemoryLayout<AudioDeviceID>.size)
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    let status = AudioObjectGetPropertyData(
        AudioObjectID(kAudioObjectSystemObject),
        &addr,
        0,
        nil,
        &size,
        &deviceID
    )
    guard status == noErr, deviceID != 0 else { return false }
    return isDeviceRunningSomewhere(deviceID)
}

func isAnyInputDeviceRunning() -> Bool {
    if isDefaultMicRunning() {
        return true
    }
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    var dataSize: UInt32 = 0
    let sizeStatus = AudioObjectGetPropertyDataSize(
        AudioObjectID(kAudioObjectSystemObject),
        &addr,
        0,
        nil,
        &dataSize
    )
    guard sizeStatus == noErr, dataSize > 0 else { return false }
    let count = Int(dataSize) / MemoryLayout<AudioDeviceID>.size
    var devices = [AudioDeviceID](repeating: 0, count: count)
    let status = AudioObjectGetPropertyData(
        AudioObjectID(kAudioObjectSystemObject),
        &addr,
        0,
        nil,
        &dataSize,
        &devices
    )
    guard status == noErr else { return false }
    for dev in devices {
        if dev != 0 && deviceHasInputChannels(dev) && isDeviceRunningSomewhere(dev) {
            return true
        }
    }
    return false
}

func computeDHash64(from cgImage: CGImage) -> UInt64 {
    let w = 9
    let h = 8
    var pixels = [UInt8](repeating: 0, count: w * h)
    let colorSpace = CGColorSpaceCreateDeviceGray()
    guard let ctx = CGContext(
        data: &pixels,
        width: w,
        height: h,
        bitsPerComponent: 8,
        bytesPerRow: w,
        space: colorSpace,
        bitmapInfo: CGImageAlphaInfo.none.rawValue
    ) else { return 0 }

    ctx.interpolationQuality = .low
    ctx.draw(cgImage, in: CGRect(x: 0, y: 0, width: w, height: h))

    var hash: UInt64 = 0
    var bit = 0
    for row in 0..<8 {
        for col in 0..<8 {
            let left = pixels[row * w + col]
            let right = pixels[row * w + col + 1]
            if left > right {
                hash |= (UInt64(1) << UInt64(bit))
            }
            bit += 1
        }
    }
    return hash
}

func runVisionOCR(on cgImage: CGImage) -> String {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.recognitionLanguages = ["de-DE", "en-US"]
    request.usesLanguageCorrection = true

    let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
    do {
        try handler.perform([request])
        let lines = request.results?.compactMap { $0.topCandidates(1).first?.string } ?? []
        return lines.joined(separator: "\n")
    } catch {
        return ""
    }
}

func jpegBase64(from cgImage: CGImage, maxWidth: CGFloat = 1120) -> String? {
    let origW = CGFloat(cgImage.width)
    let origH = CGFloat(cgImage.height)
    let scale = min(1.0, maxWidth / max(1.0, origW))
    let targetW = Int(origW * scale)
    let targetH = Int(origH * scale)
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: targetW,
        pixelsHigh: targetH,
        bitsPerSample: 8,
        samplesPerPixel: 4,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: .deviceRGB,
        bytesPerRow: 0,
        bitsPerPixel: 0
    )
    guard let bitmap = rep else { return nil }
    NSGraphicsContext.saveGraphicsState()
    if let ctx = NSGraphicsContext(bitmapImageRep: bitmap) {
        NSGraphicsContext.current = ctx
        let nsImg = NSImage(cgImage: cgImage, size: NSSize(width: targetW, height: targetH))
        nsImg.draw(in: NSRect(x: 0, y: 0, width: targetW, height: targetH))
    }
    NSGraphicsContext.restoreGraphicsState()
    guard let jpg = bitmap.representation(using: .jpeg, properties: [.compressionFactor: 0.72]) else {
        return nil
    }
    return "data:image/jpeg;base64," + jpg.base64EncodedString()
}

func axStringAttr(_ el: AXUIElement, _ attr: String) -> String? {
    var val: AnyObject?
    let err = AXUIElementCopyAttributeValue(el, attr as CFString, &val)
    guard err == .success else { return nil }
    if let s = val as? String { return s }
    if let url = val as? URL { return url.absoluteString }
    return nil
}

func collectAXText(_ el: AXUIElement, depth: Int, maxDepth: Int, buffer: inout [String], seenChars: inout Int, secureDetected: inout Bool) {
    if depth > maxDepth || seenChars > 3800 { return }
    if let role = axStringAttr(el, kAXRoleAttribute) {
        if role == "AXSecureTextField" {
            secureDetected = true
            return
        }
    }
    for attr in [kAXValueAttribute, kAXTitleAttribute, kAXDescriptionAttribute] {
        if let txt = axStringAttr(el, attr)?.trimmingCharacters(in: .whitespacesAndNewlines),
           txt.count >= 2 && txt.count <= 800 {
            if buffer.last != txt {
                buffer.append(txt)
                seenChars += txt.count
            }
        }
    }
    var childrenRef: AnyObject?
    if AXUIElementCopyAttributeValue(el, kAXChildrenAttribute as CFString, &childrenRef) == .success,
       let children = childrenRef as? [AXUIElement] {
        for child in children.prefix(35) {
            if seenChars > 3800 { break }
            collectAXText(child, depth: depth + 1, maxDepth: maxDepth, buffer: &buffer, seenChars: &seenChars, secureDetected: &secureDetected)
        }
    }
}

func queryBrowserURL(bundleID: String, appName: String) -> String? {
    let script: String
    if bundleID == "com.apple.Safari" || bundleID == "com.kagi.kagimacOS" {
        script = "tell application \"\(appName)\" to if (count of documents) > 0 then return URL of front document"
    } else if [
        "com.google.Chrome",
        "org.chromium.Chromium",
        "company.thebrowser.Browser",
        "company.thebrowser.dia",
        "com.brave.Browser",
        "com.microsoft.edgemac",
        "com.vivaldi.Vivaldi",
        "com.operasoftware.Opera"
    ].contains(bundleID) {
        script = "tell application \"\(appName)\" to if (count of windows) > 0 then return URL of active tab of front window"
    } else {
        return nil
    }
    let proc = Process()
    proc.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
    proc.arguments = ["-e", script]
    let pipe = Pipe()
    proc.standardOutput = pipe
    proc.standardError = Pipe()
    try? proc.run()
    proc.waitUntilExit()
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    let s = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    return s.isEmpty ? nil : s
}

func frontmostCGWindowTitle(for pid: pid_t) -> String {
    guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else {
        return ""
    }
    var bestTitle = ""
    var bestArea: Double = 0.0
    for win in list {
        let ownerPid = (win[kCGWindowOwnerPID as String] as? Int32) ?? 0
        let layer = (win[kCGWindowLayer as String] as? Int) ?? 0
        if ownerPid != pid || layer != 0 { continue }
        let name = (win[kCGWindowName as String] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        if name.isEmpty { continue }
        let bounds = win[kCGWindowBounds as String] as? [String: Any]
        let w = (bounds?["Width"] as? Double) ?? 0.0
        let h = (bounds?["Height"] as? Double) ?? 0.0
        let area = w * h
        if area >= bestArea {
            bestArea = area
            bestTitle = name
        }
    }
    return bestTitle
}

func handlePermissions() {
    let ax = AXIsProcessTrusted()
    let sck = CGPreflightScreenCaptureAccess()
    let mic = AVCaptureDevice.authorizationStatus(for: .audio) == .authorized
    let calStatus = EKEventStore.authorizationStatus(for: .event)
    let cal: Bool
    if #available(macOS 14.0, *) {
        cal = (calStatus == .fullAccess || calStatus == .authorized)
    } else {
        cal = (calStatus == .authorized)
    }
    let idleSecs = CGEventSource.secondsSinceLastEventType(.combinedSessionState, eventType: .null)
    let sessionDict = CGSessionCopyCurrentDictionary() as? [String: Any]
    let screenLocked = (sessionDict?["CGSSessionScreenIsLocked"] as? Bool) ?? false

    let payload: [String: Any] = [
        "platform": "macos",
        "screen_capture_granted": sck,
        "microphone_granted": mic,
        "accessibility_granted": ax,
        "calendar_granted": cal,
        "idle_seconds": Int(idleSecs),
        "screen_locked": screenLocked
    ]
    JSONOut.emit(payload)
}

func handleOpenSettings(_ kind: String) {
    let urlStr: String
    switch kind {
    case "screen":
        urlStr = "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
    case "microphone":
        urlStr = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
    case "accessibility":
        urlStr = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
    case "calendar":
        urlStr = "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars"
    default:
        urlStr = "x-apple.systempreferences:com.apple.preference.security?Privacy"
    }
    if let url = URL(string: urlStr) {
        NSWorkspace.shared.open(url)
    }
    JSONOut.emit(["opened": urlStr])
}

func handleRequestPermission(_ kind: String) {
    switch kind {
    case "screen":
        _ = CGRequestScreenCaptureAccess()
        handleOpenSettings("screen")
    case "accessibility":
        let opts = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(opts)
    case "microphone":
        let sem = DispatchSemaphore(value: 0)
        AVCaptureDevice.requestAccess(for: .audio) { _ in sem.signal() }
        _ = sem.wait(timeout: .now() + 10)
    case "calendar":
        let store = EKEventStore()
        let sem = DispatchSemaphore(value: 0)
        if #available(macOS 14.0, *) {
            store.requestFullAccessToEvents { _, _ in sem.signal() }
        } else {
            store.requestAccess(to: .event) { _, _ in sem.signal() }
        }
        _ = sem.wait(timeout: .now() + 10)
    default:
        handleOpenSettings(kind)
    }
    handlePermissions()
}

func handleActiveWindow() {
    guard let front = NSWorkspace.shared.frontmostApplication else {
        JSONOut.emit(["app_name": "macOS", "app_bundle_id": "unknown", "window_title": ""])
        return
    }
    let bundleID = front.bundleIdentifier ?? "unknown"
    let appName = front.localizedName ?? "macOS App"
    let pid = front.processIdentifier

    var windowTitle = ""
    var secureFocused = false
    var textChunks: [String] = []
    var seenChars = 0

    if AXIsProcessTrusted() {
        let appEl = AXUIElementCreateApplication(pid)
        var focusedWinRef: AnyObject?
        if AXUIElementCopyAttributeValue(appEl, kAXFocusedWindowAttribute as CFString, &focusedWinRef) == .success,
           let winEl = focusedWinRef {
            let axWin = winEl as! AXUIElement
            windowTitle = axStringAttr(axWin, kAXTitleAttribute) ?? ""
            collectAXText(axWin, depth: 0, maxDepth: 6, buffer: &textChunks, seenChars: &seenChars, secureDetected: &secureFocused)
        }
        var focusedElRef: AnyObject?
        if AXUIElementCopyAttributeValue(appEl, kAXFocusedUIElementAttribute as CFString, &focusedElRef) == .success,
           let focEl = focusedElRef {
            let axFoc = focEl as! AXUIElement
            if axStringAttr(axFoc, kAXRoleAttribute) == "AXSecureTextField" {
                secureFocused = true
            }
        }
    }

    if windowTitle.isEmpty {
        windowTitle = frontmostCGWindowTitle(for: pid)
    }

    let lowTitle = windowTitle.lowercased()
    let isPrivate = lowTitle.contains("private") || lowTitle.contains("inkognito") || lowTitle.contains("incognito") || lowTitle.contains("privat")
    let browserURL = isPrivate ? nil : queryBrowserURL(bundleID: bundleID, appName: appName)

    var activeSpeaker: String? = nil
    var presenter: String? = nil
    let joinedText = textChunks.joined(separator: " · ")

    if lowTitle.contains("zoom") || lowTitle.contains("meet") || lowTitle.contains("teams") {
        for chunk in textChunks {
            if chunk.lowercased().contains("is presenting") || chunk.lowercased().contains("präsentiert") {
                presenter = chunk
            } else if chunk.lowercased().contains("speaking:") || chunk.lowercased().contains("spricht") {
                activeSpeaker = chunk
                    .replacingOccurrences(of: "Speaking:", with: "")
                    .replacingOccurrences(of: "spricht", with: "")
                    .trimmingCharacters(in: .whitespacesAndNewlines)
            }
        }
    }

    let payload: [String: Any] = [
        "window_id": Int(pid),
        "app_bundle_id": bundleID,
        "app_name": appName,
        "window_title": windowTitle,
        "browser_url": jsonNullable(browserURL),
        "visible_text": secureFocused || isPrivate ? "" : (joinedText.isEmpty ? "" : joinedText),
        "is_private_browsing": isPrivate,
        "is_secure_field_focused": secureFocused,
        "detected_screen_share_presenter": jsonNullable(presenter),
        "detected_active_speaker_tile": jsonNullable(activeSpeaker),
        "mic_running": isAnyInputDeviceRunning()
    ]
    JSONOut.emit(payload)
}

@available(macOS 14.0, *)
func captureFrontmostSCImage() async -> CGImage? {
    guard CGPreflightScreenCaptureAccess() else { return nil }
    do {
        let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        guard let frontPID = NSWorkspace.shared.frontmostApplication?.processIdentifier else { return nil }
        let candidates = content.windows.filter {
            $0.owningApplication?.processID == frontPID
                && $0.windowLayer == 0
                && $0.frame.width > 160
                && $0.frame.height > 120
        }
        guard let targetWin = candidates.max(by: {
            ($0.frame.width * $0.frame.height) < ($1.frame.width * $1.frame.height)
        }) else {
            return nil
        }
        let filter = SCContentFilter(desktopIndependentWindow: targetWin)
        let cfg = SCStreamConfiguration()
        cfg.width = min(Int(targetWin.frame.width) * 2, 1920)
        cfg.height = min(Int(targetWin.frame.height) * 2, 1200)
        cfg.showsCursor = false
        return try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: cfg)
    } catch {
        return nil
    }
}

func handleCaptureWindowOCR(includeImage: Bool) {
    let sem = DispatchSemaphore(value: 0)
    var captured: CGImage? = nil

    if #available(macOS 14.0, *) {
        Task {
            captured = await captureFrontmostSCImage()
            sem.signal()
        }
        _ = sem.wait(timeout: .now() + 5)
    }

    guard let img = captured else {
        JSONOut.emit(["ok": false, "dhash_hex": "0000000000000000", "ocr_text": ""])
        return
    }
    let dhash = computeDHash64(from: img)
    let ocr = runVisionOCR(on: img)
    let dataURL = includeImage ? jpegBase64(from: img) : nil

    let payload: [String: Any] = [
        "ok": true,
        "dhash_u64": dhash,
        "dhash_hex": String(format: "%016llx", dhash),
        "ocr_text": ocr,
        "image_data_url": jsonNullable(dataURL)
    ]
    JSONOut.emit(payload)
}

func handleClipboard() {
    let pb = NSPasteboard.general
    let types = pb.types?.map { $0.rawValue } ?? []
    let concealed = types.contains("org.nspasteboard.ConcealedType")
        || types.contains("org.nspasteboard.TransientType")
        || types.contains("org.nspasteboard.AutoGeneratedType")
        || types.contains("com.agilebits.1password")
        || types.contains("com.bitwarden.desktop")
    let text = concealed ? "" : (pb.string(forType: .string) ?? "")
    let front = NSWorkspace.shared.frontmostApplication?.localizedName ?? "macOS"
    let payload: [String: Any] = [
        "change_count": pb.changeCount,
        "source_app": front,
        "text": text,
        "is_concealed": concealed
    ]
    JSONOut.emit(payload)
}

func extractConferenceURL(from texts: [String?]) -> String? {
    let joined = texts.compactMap { $0 }.joined(separator: " ")
    let patterns = [
        "https://[a-zA-Z0-9.-]*zoom\\.us/j/[0-9a-zA-Z?=&_-]+",
        "https://meet\\.google\\.com/[a-z-]+",
        "https://teams\\.microsoft\\.com/l/meetup-join/[^\\s\"<>]+"
    ]
    for pat in patterns {
        if let regex = try? NSRegularExpression(pattern: pat),
           let match = regex.firstMatch(in: joined, range: NSRange(joined.startIndex..., in: joined)),
           let range = Range(match.range, in: joined) {
            return String(joined[range])
        }
    }
    return nil
}

func handleCalendar() {
    let store = EKEventStore()
    let status = EKEventStore.authorizationStatus(for: .event)
    let authorized: Bool
    if #available(macOS 14.0, *) {
        authorized = (status == .fullAccess || status == .authorized)
    } else {
        authorized = (status == .authorized)
    }
    guard authorized else {
        JSONOut.emit(["authorized": false, "events": []])
        return
    }

    let now = Date()
    let start = now.addingTimeInterval(-1800)
    let end = now.addingTimeInterval(86400)
    let pred = store.predicateForEvents(withStart: start, end: end, calendars: nil)
    let events = store.events(matching: pred).filter { !$0.isAllDay }
    let iso = ISO8601DateFormatter()

    let mapped: [[String: Any]] = events.prefix(25).map { ev in
        let attendees: [[String: String]] = (ev.attendees ?? []).map { att in
            let email = att.url.absoluteString.replacingOccurrences(of: "mailto:", with: "")
            return ["name": att.name ?? email, "email": email]
        }
        let confURL = extractConferenceURL(from: [ev.url?.absoluteString, ev.location, ev.notes])
        return [
            "event_id": ev.eventIdentifier ?? UUID().uuidString,
            "title": ev.title ?? "Meeting",
            "starts_at": iso.string(from: ev.startDate),
            "ends_at": iso.string(from: ev.endDate),
            "conference_url": jsonNullable(confURL),
            "attendees": attendees
        ]
    }
    JSONOut.emit(["authorized": true, "events": mapped])
}

func writeMono16kWav(samples: [Int16], to path: String) throws {
    let sampleCount: Int = samples.count
    var data = Data()
    let sampleRate: UInt32 = 16000
    let numChannels: UInt16 = 1
    let bitsPerSample: UInt16 = 16
    let dataSize = UInt32(sampleCount * 2)
    let chunkSize = 36 + dataSize
    let byteRate = sampleRate * UInt32(numChannels) * UInt32(bitsPerSample / 8)
    let blockAlign = numChannels * (bitsPerSample / 8)

    func appendLE<T: FixedWidthInteger>(_ val: T) {
        var le = val.littleEndian
        data.append(Data(bytes: &le, count: MemoryLayout<T>.size))
    }

    data.append("RIFF".data(using: .ascii)!)
    appendLE(chunkSize)
    data.append("WAVE".data(using: .ascii)!)
    data.append("fmt ".data(using: .ascii)!)
    appendLE(UInt32(16))
    appendLE(UInt16(1)) // PCM
    appendLE(numChannels)
    appendLE(sampleRate)
    appendLE(byteRate)
    appendLE(blockAlign)
    appendLE(bitsPerSample)
    data.append("data".data(using: .ascii)!)
    appendLE(dataSize)
    if sampleCount > 0 {
        samples.withUnsafeBufferPointer { buf in
            if let base = buf.baseAddress {
                data.append(UnsafeBufferPointer(start: UnsafeRawPointer(base).assumingMemoryBound(to: UInt8.self), count: sampleCount * 2))
            }
        }
    }
    try data.write(to: URL(fileURLWithPath: path))
}

func computeRmsNormalized(_ samples: [Int16]) -> Double {
    let n: Int = samples.count
    if n == 0 { return 0.0 }
    var sumSq: Double = 0.0
    for s in samples {
        let f = Double(s) / 32768.0
        sumSq += f * f
    }
    let rms = sqrt(sumSq / Double(n))
    return min(1.0, rms * 3.5)
}

final class ContinuousMicRecorder {
    private let engine = AVAudioEngine()
    private let lock = NSLock()
    private var buffer: [Int16] = []
    private var latestRms: Double = 0.0
    private(set) var isRunning: Bool = false
    private(set) var errorMessage: String? = nil

    func start() {
        let auth = AVCaptureDevice.authorizationStatus(for: .audio)
        if auth == .denied || auth == .restricted {
            errorMessage = "Mikrofon-Berechtigung in macOS Systemeinstellungen nicht erteilt."
            return
        }
        let input = engine.inputNode
        let inputFmt = input.outputFormat(forBus: 0)
        if inputFmt.sampleRate <= 0 || inputFmt.channelCount == 0 {
            errorMessage = "Kein aktives Mikrofon-Eingabegerät gefunden."
            return
        }
        guard let targetFmt = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16000, channels: 1, interleaved: true),
              let converter = AVAudioConverter(from: inputFmt, to: targetFmt) else {
            errorMessage = "Audio-Konverter konnte nicht initialisiert werden."
            return
        }

        input.installTap(onBus: 0, bufferSize: 2048, format: inputFmt) { [weak self] pcmBuf, _ in
            guard let self = self else { return }
            let ratio = 16000.0 / max(1.0, inputFmt.sampleRate)
            let outFrames = AVAudioFrameCount(Double(pcmBuf.frameLength) * ratio) + 64
            guard let outBuf = AVAudioPCMBuffer(pcmFormat: targetFmt, frameCapacity: outFrames) else { return }
            var consumed = false
            var err: NSError?
            converter.convert(to: outBuf, error: &err) { _, outStatus in
                if consumed {
                    outStatus.pointee = .noDataNow
                    return nil
                }
                consumed = true
                outStatus.pointee = .haveData
                return pcmBuf
            }
            if err == nil, let ptr = outBuf.int16ChannelData?[0] {
                let count = Int(outBuf.frameLength)
                let slice = Array(UnsafeBufferPointer(start: ptr, count: count))
                let rms = computeRmsNormalized(slice)
                self.lock.lock()
                self.buffer.append(contentsOf: slice)
                self.latestRms = rms
                self.lock.unlock()
            }
        }

        do {
            try engine.start()
            isRunning = true
        } catch {
            errorMessage = "Mikrofon-Start fehlgeschlagen: \(error.localizedDescription)"
            isRunning = false
        }
    }

    func stop() {
        if isRunning {
            engine.inputNode.removeTap(onBus: 0)
            engine.stop()
            isRunning = false
        }
    }

    func currentRms() -> Double {
        lock.lock()
        defer { lock.unlock() }
        return latestRms
    }

    func drainSamples() -> [Int16] {
        lock.lock()
        let out = buffer
        buffer.removeAll(keepingCapacity: true)
        lock.unlock()
        return out
    }
}

@available(macOS 13.0, *)
final class SystemAudioCollector: NSObject, SCStreamOutput {
    private let lock = NSLock()
    private var samples: [Int16] = []
    private var latestRms: Double = 0.0

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio, sampleBuffer.isValid else { return }
        guard let blockBuf = CMSampleBufferGetDataBuffer(sampleBuffer) else { return }

        var sampleRate: Double = 48000.0
        var channels: Int = 1
        var isNonInterleaved: Bool = false
        if let fmtDesc = CMSampleBufferGetFormatDescription(sampleBuffer),
           let asbdPtr = CMAudioFormatDescriptionGetStreamBasicDescription(fmtDesc) {
            let asbd = asbdPtr.pointee
            if asbd.mSampleRate > 1000.0 {
                sampleRate = asbd.mSampleRate
            }
            channels = max(1, Int(asbd.mChannelsPerFrame))
            isNonInterleaved = (asbd.mFormatFlags & kAudioFormatFlagIsNonInterleaved) != 0
        }

        var totalLen = 0
        var dataPtr: UnsafeMutablePointer<Int8>?
        guard CMBlockBufferGetDataPointer(blockBuf, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: &totalLen, dataPointerOut: &dataPtr) == kCMBlockBufferNoErr,
              let ptr = dataPtr else { return }

        let floatCount = totalLen / MemoryLayout<Float>.size
        let floatPtr = UnsafeRawPointer(ptr).assumingMemoryBound(to: Float.self)
        let decimate = max(1, Int(round(sampleRate / 16000.0)))
        let frameCount = floatCount / channels
        let strideStep = isNonInterleaved ? decimate : (decimate * channels)

        var chunk: [Int16] = []
        chunk.reserveCapacity(max(1, frameCount / decimate))
        var i = 0
        let limit = isNonInterleaved ? frameCount : floatCount
        while i < limit {
            let sampleVal = floatPtr[i]
            let clamped = max(-1.0, min(1.0, sampleVal))
            chunk.append(Int16(clamped * 32767.0))
            i += strideStep
        }

        let rms = computeRmsNormalized(chunk)
        lock.lock()
        samples.append(contentsOf: chunk)
        latestRms = rms
        lock.unlock()
    }

    func currentRms() -> Double {
        lock.lock()
        defer { lock.unlock() }
        return latestRms
    }

    func drainSamples() -> [Int16] {
        lock.lock()
        let out = samples
        samples.removeAll(keepingCapacity: true)
        lock.unlock()
        return out
    }

    func snapshot() -> [Int16] {
        lock.lock()
        defer { lock.unlock() }
        return samples
    }
}

func recordMicrophoneSamples(durationSecs: Double) -> [Int16] {
    let rec = ContinuousMicRecorder()
    rec.start()
    guard rec.isRunning else { return [] }
    Thread.sleep(forTimeInterval: durationSecs)
    rec.stop()
    return rec.drainSamples()
}

func handleRecordDualAudio(outDir: String, seconds: Double) {
    try? FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true)
    let micPath = (outDir as NSString).appendingPathComponent("mic.wav")
    let sysPath = (outDir as NSString).appendingPathComponent("system.wav")

    var micSamples: [Int16] = []
    var sysSamples: [Int16] = []
    let group = DispatchGroup()

    group.enter()
    DispatchQueue.global(qos: .userInitiated).async {
        micSamples = recordMicrophoneSamples(durationSecs: seconds)
        group.leave()
    }

    if #available(macOS 13.0, *), CGPreflightScreenCaptureAccess() {
        group.enter()
        Task {
            do {
                let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
                if let display = content.displays.first {
                    let filter = SCContentFilter(display: display, excludingApplications: [], exceptingWindows: [])
                    let cfg = SCStreamConfiguration()
                    cfg.capturesAudio = true
                    cfg.excludesCurrentProcessAudio = true
                    cfg.sampleRate = 48000
                    cfg.channelCount = 1
                    cfg.width = 2
                    cfg.height = 2
                    let collector = SystemAudioCollector()
                    let stream = SCStream(filter: filter, configuration: cfg, delegate: nil)
                    try stream.addStreamOutput(collector, type: .audio, sampleHandlerQueue: DispatchQueue(label: "ai.backtext.sck.audio"))
                    try await stream.startCapture()
                    try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
                    try await stream.stopCapture()
                    sysSamples = collector.snapshot()
                }
            } catch {
                // Non-fatal fallback
            }
            group.leave()
        }
    }

    group.wait()
    let micCount: Int = micSamples.count
    let sysCount: Int = sysSamples.count
    let micOk = (try? writeMono16kWav(samples: micSamples, to: micPath)) != nil && micCount > 0
    let sysOk = (try? writeMono16kWav(samples: sysSamples, to: sysPath)) != nil && sysCount > 0
    let payload: [String: Any] = [
        "mic_wav_path": jsonNullable(micOk ? micPath : nil),
        "mic_samples": micCount,
        "system_wav_path": jsonNullable(sysOk ? sysPath : nil),
        "system_samples": sysCount
    ]
    JSONOut.emit(payload)
}

func handleStreamDualAudio(outDir: String, chunkSecs: Double) {
    try? FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true)
    let effectiveChunkSecs = max(3.0, min(30.0, chunkSecs))

    let stopLock = NSLock()
    var shouldStop = false
    func requestStop() {
        stopLock.lock()
        shouldStop = true
        stopLock.unlock()
    }
    func isStopping() -> Bool {
        stopLock.lock()
        defer { stopLock.unlock() }
        return shouldStop
    }

    // Listen for stdin EOF or "stop" command from parent Rust process
    DispatchQueue.global(qos: .utility).async {
        while true {
            let data = FileHandle.standardInput.availableData
            if data.isEmpty {
                requestStop()
                break
            }
            if let text = String(data: data, encoding: .utf8), text.lowercased().contains("stop") {
                requestStop()
                break
            }
        }
    }

    // Handle SIGTERM cleanly so final audio chunk is flushed
    signal(SIGTERM, SIG_IGN)
    let sigTermSrc = DispatchSource.makeSignalSource(signal: SIGTERM, queue: DispatchQueue.global())
    sigTermSrc.setEventHandler { requestStop() }
    sigTermSrc.resume()

    let micRec = ContinuousMicRecorder()
    micRec.start()

    var sysCollector: AnyObject? = nil
    var sysStream: AnyObject? = nil
    var sysActive = false
    var sysError: String? = nil

    if #available(macOS 13.0, *) {
        if CGPreflightScreenCaptureAccess() {
            let sem = DispatchSemaphore(value: 0)
            Task {
                do {
                    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
                    if let display = content.displays.first {
                        let filter = SCContentFilter(display: display, excludingApplications: [], exceptingWindows: [])
                        let cfg = SCStreamConfiguration()
                        cfg.capturesAudio = true
                        cfg.excludesCurrentProcessAudio = true
                        cfg.sampleRate = 48000
                        cfg.channelCount = 1
                        cfg.width = 2
                        cfg.height = 2
                        let collector = SystemAudioCollector()
                        let stream = SCStream(filter: filter, configuration: cfg, delegate: nil)
                        try stream.addStreamOutput(collector, type: .audio, sampleHandlerQueue: DispatchQueue(label: "ai.backtext.sck.stream"))
                        try await stream.startCapture()
                        sysCollector = collector
                        sysStream = stream
                        sysActive = true
                    }
                } catch {
                    sysError = "System-Audio (ScreenCaptureKit): \(error.localizedDescription)"
                }
                sem.signal()
            }
            _ = sem.wait(timeout: .now() + 4)
        } else {
            sysError = "System-Audio benötigt Bildschirmaufnahme-Berechtigung."
        }
    }

    let startTime = Date()
    var lastFlush = Date()
    var chunkSeq: Int = 0

    func emitChunk(forceFinal: Bool) {
        let micSamples = micRec.drainSamples()
        var sysSamples: [Int16] = []
        if #available(macOS 13.0, *), let collector = sysCollector as? SystemAudioCollector {
            sysSamples = collector.drainSamples()
        }
        let micCount: Int = micSamples.count
        let sysCount: Int = sysSamples.count
        if forceFinal && micCount < 8000 && sysCount < 8000 {
            return
        }
        if micCount == 0 && sysCount == 0 {
            return
        }
        chunkSeq += 1
        let micPath = (outDir as NSString).appendingPathComponent("chunk_\(chunkSeq)_mic.wav")
        let sysPath = (outDir as NSString).appendingPathComponent("chunk_\(chunkSeq)_system.wav")
        let micOk = micCount > 0 && (try? writeMono16kWav(samples: micSamples, to: micPath)) != nil
        let sysOk = sysCount > 0 && (try? writeMono16kWav(samples: sysSamples, to: sysPath)) != nil

        let payload: [String: Any] = [
            "type": "chunk",
            "seq": chunkSeq,
            "mic_wav_path": jsonNullable(micOk ? micPath : nil),
            "mic_samples": micCount,
            "system_wav_path": jsonNullable(sysOk ? sysPath : nil),
            "system_samples": sysCount
        ]
        JSONOut.emit(payload)
    }

    var speechMsSinceFlush: Double = 0.0
    var silenceAfterSpeechMs: Double = 0.0
    var loopIter: Int = 0

    while !isStopping() {
        Thread.sleep(forTimeInterval: 0.2)
        loopIter += 1
        let elapsed = Int(Date().timeIntervalSince(startTime))
        let micRms = micRec.currentRms()
        var sysRms = 0.0
        if #available(macOS 13.0, *), let collector = sysCollector as? SystemAudioCollector {
            sysRms = collector.currentRms()
        }

        let peakRms = max(micRms, sysRms)
        if peakRms >= 0.045 {
            speechMsSinceFlush += 200.0
            silenceAfterSpeechMs = 0.0
        } else if speechMsSinceFlush > 0.0 {
            silenceAfterSpeechMs += 200.0
        }

        if loopIter % 2 == 0 {
            let combinedErr: String? = micRec.errorMessage ?? sysError
            let levelPayload: [String: Any] = [
                "type": "level",
                "mic_rms": micRms,
                "system_rms": sysRms,
                "mic_active": micRec.isRunning,
                "system_active": sysActive,
                "elapsed_secs": elapsed,
                "error": jsonNullable(combinedErr)
            ]
            JSONOut.emit(levelPayload)
        }

        let sinceFlush = Date().timeIntervalSince(lastFlush)
        let naturalPauseFlush = speechMsSinceFlush >= 1200.0 && silenceAfterSpeechMs >= 700.0 && sinceFlush >= 1.8
        let maxDurationFlush = sinceFlush >= effectiveChunkSecs
        if naturalPauseFlush || maxDurationFlush {
            lastFlush = Date()
            speechMsSinceFlush = 0.0
            silenceAfterSpeechMs = 0.0
            emitChunk(forceFinal: false)
        }
    }

    // Final flush & clean shutdown
    micRec.stop()
    if #available(macOS 13.0, *), let stream = sysStream as? SCStream {
        let sem = DispatchSemaphore(value: 0)
        Task {
            try? await stream.stopCapture()
            sem.signal()
        }
        _ = sem.wait(timeout: .now() + 2)
    }
    emitChunk(forceFinal: true)
    sigTermSrc.cancel()
}

func matchMeetingFromStrings(bundleID: String, ownerName: String, title: String, url: String) -> (String, String)? {
    let bLow = bundleID.lowercased()
    let oLow = ownerName.lowercased()
    let tLow = title.lowercased()
    let uLow = url.lowercased()
    let combined = "\(bLow) \(oLow) \(tLow) \(uLow)"

    if bLow.contains("com.microsoft.teams") || oLow.contains("microsoft teams") {
        let cleanTitle = title.isEmpty ? "Microsoft Teams Meeting" : title
        return ("Microsoft Teams", cleanTitle)
    }
    if uLow.contains("teams.microsoft")
        || uLow.contains("teams.live")
        || uLow.contains("teams.cloud.microsoft")
        || tLow.contains("microsoft teams")
        || tLow.contains("teams.microsoft")
        || tLow.contains("teams.live")
    {
        let cleanTitle = title.isEmpty ? "Microsoft Teams (Browser)" : title
        return ("Microsoft Teams (Browser)", cleanTitle)
    }
    if bLow == "us.zoom.xos"
        || uLow.contains("zoom.us/j/")
        || uLow.contains("zoom.us/wc/")
        || tLow.contains("zoom meeting")
        || tLow.contains("zoom workplace")
    {
        let cleanTitle = title.isEmpty ? "Zoom Meeting" : title
        return ("Zoom", cleanTitle)
    }
    if uLow.contains("meet.google.com/")
        || tLow.contains("google meet")
        || tLow.hasPrefix("meet - ")
        || tLow.hasPrefix("meet – ")
    {
        let cleanTitle = title.isEmpty ? "Google Meet" : title
        return ("Google Meet", cleanTitle)
    }
    if (bLow.contains("slack") || oLow.contains("slack")) && tLow.contains("huddle") {
        let cleanTitle = title.isEmpty ? "Slack Huddle" : title
        return ("Slack Huddle", cleanTitle)
    }
    if bLow == "com.apple.facetime" || oLow == "facetime" {
        let cleanTitle = title.isEmpty ? "FaceTime" : title
        return ("FaceTime", cleanTitle)
    }
    if combined.contains("webex") {
        let cleanTitle = title.isEmpty ? "Cisco Webex" : title
        return ("Cisco Webex", cleanTitle)
    }
    if uLow.contains("whereby.com") || tLow.contains("whereby") {
        let cleanTitle = title.isEmpty ? "Whereby" : title
        return ("Whereby", cleanTitle)
    }
    if uLow.contains("meet.jit.si") || tLow.contains("jitsi meet") {
        let cleanTitle = title.isEmpty ? "Jitsi Meet" : title
        return ("Jitsi Meet", cleanTitle)
    }
    return nil
}

func scanBrowserMeetingTab(runningApps: [NSRunningApplication]) -> (String, String)? {
    let chromiumBundles: [(String, String)] = [
        ("com.google.Chrome", "Google Chrome"),
        ("company.thebrowser.Browser", "Arc"),
        ("com.microsoft.edgemac", "Microsoft Edge"),
        ("com.brave.Browser", "Brave Browser"),
        ("com.vivaldi.Vivaldi", "Vivaldi"),
        ("org.chromium.Chromium", "Chromium")
    ]
    let runningBundles = Set(runningApps.compactMap { $0.bundleIdentifier })

    func runOsa(_ script: String) -> String {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        proc.arguments = ["-e", script]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = Pipe()
        try? proc.run()
        proc.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        return String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    }

    for (bid, appName) in chromiumBundles {
        if !runningBundles.contains(bid) { continue }
        let script = """
        tell application "\(appName)"
            repeat with w in windows
                repeat with t in tabs of w
                    set u to URL of t
                    if u contains "teams.microsoft" or u contains "teams.live" or u contains "teams.cloud.microsoft" or u contains "meet.google.com/" or u contains "zoom.us/j/" or u contains "zoom.us/wc/" or u contains "whereby.com" or u contains "meet.jit.si" then
                        return (title of t) & "|||" & u
                    end if
                end repeat
            end repeat
        end tell
        """
        let res = runOsa(script)
        if !res.isEmpty {
            let parts = res.components(separatedBy: "|||")
            let tabTitle = parts.first ?? ""
            let tabUrl = parts.count > 1 ? parts[1] : ""
            if let matched = matchMeetingFromStrings(bundleID: bid, ownerName: appName, title: tabTitle, url: tabUrl) {
                return matched
            }
        }
    }

    if runningBundles.contains("com.apple.Safari") {
        let script = """
        tell application "Safari"
            repeat with w in windows
                repeat with t in tabs of w
                    set u to URL of t
                    if u contains "teams.microsoft" or u contains "teams.live" or u contains "teams.cloud.microsoft" or u contains "meet.google.com/" or u contains "zoom.us/j/" or u contains "zoom.us/wc/" or u contains "whereby.com" or u contains "meet.jit.si" then
                        return (name of t) & "|||" & u
                    end if
                end repeat
            end repeat
        end tell
        """
        let res = runOsa(script)
        if !res.isEmpty {
            let parts = res.components(separatedBy: "|||")
            let tabTitle = parts.first ?? ""
            let tabUrl = parts.count > 1 ? parts[1] : ""
            if let matched = matchMeetingFromStrings(bundleID: "com.apple.Safari", ownerName: "Safari", title: tabTitle, url: tabUrl) {
                return matched
            }
        }
    }

    return nil
}

func handleAudioEnvironment() {
    let micRunning: Bool = isAnyInputDeviceRunning()
    let runningApps = NSWorkspace.shared.runningApplications

    var pidMap: [Int32: (String, String)] = [:]
    for app in runningApps {
        pidMap[app.processIdentifier] = (app.bundleIdentifier ?? "", app.localizedName ?? "")
    }

    var detectedApp: String? = nil
    var detectedTitle: String? = nil
    var hasCallControls: Bool = false

    // 1. Scan ALL on-screen windows (not just frontmost!) — catches Teams Desktop, Teams Browser, Zoom, Meet, Slack Huddle
    if let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] {
        var teamsWindowCount: Int = 0
        for win in list {
            let layer = (win[kCGWindowLayer as String] as? Int) ?? 0
            if layer != 0 { continue }
            let ownerPid = (win[kCGWindowOwnerPID as String] as? Int32) ?? 0
            let info = pidMap[ownerPid]
            let bid = info?.0 ?? ""
            let ownerName = (win[kCGWindowOwnerName as String] as? String) ?? (info?.1 ?? "")
            let winTitle = (win[kCGWindowName as String] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            let bounds = win[kCGWindowBounds as String] as? [String: Any]
            let width = (bounds?["Width"] as? Double) ?? 0.0
            let height = (bounds?["Height"] as? Double) ?? 0.0
            if width < 120.0 || height < 80.0 { continue }

            let bLow = bid.lowercased()
            if bLow.contains("com.microsoft.teams") || ownerName.lowercased().contains("microsoft teams") {
                teamsWindowCount += 1
            }

            if let (appLabel, mtgTitle) = matchMeetingFromStrings(bundleID: bid, ownerName: ownerName, title: winTitle, url: "") {
                // Avoid false positives when Teams/Zoom/FaceTime main lobby sits idle without mic or active call window
                let tLow = winTitle.lowercased()
                let isDedicatedCallTitle = tLow.contains("besprechung")
                    || tLow.contains("meeting")
                    || tLow.contains("anruf")
                    || tLow.contains("call")
                    || tLow.contains("huddle")
                    || tLow.contains("meet.google")
                    || tLow.contains("meet - ")
                    || tLow.contains("meet – ")
                    || (appLabel.contains("Teams") && !winTitle.isEmpty && tLow != "microsoft teams" && tLow != "teams" && !tLow.hasPrefix("chat |") && !tLow.hasPrefix("kalender |") && !tLow.hasPrefix("calendar |"))
                if micRunning || isDedicatedCallTitle {
                    detectedApp = appLabel
                    detectedTitle = mtgTitle
                    hasCallControls = true
                    break
                }
            }
        }
        // Teams desktop opens a 2nd dedicated meeting window alongside main chat window during active calls
        if detectedApp == nil && teamsWindowCount >= 2 && micRunning {
            detectedApp = "Microsoft Teams"
            detectedTitle = "Microsoft Teams Meeting"
            hasCallControls = true
        }
    }

    // 2. If mic is active and no visible window title matched (e.g. Teams/Meet in background browser tab or another Space), scan open browser tabs
    if detectedApp == nil && micRunning {
        if let (appLabel, mtgTitle) = scanBrowserMeetingTab(runningApps: runningApps) {
            detectedApp = appLabel
            detectedTitle = mtgTitle
            hasCallControls = true
        }
    }

    // 3. Final fallback when mic is active and a dedicated native call app (Teams Desktop / Zoom / FaceTime) is running in another Space
    if detectedApp == nil && micRunning {
        for app in runningApps {
            let bid = (app.bundleIdentifier ?? "").lowercased()
            if bid.contains("com.microsoft.teams") {
                detectedApp = "Microsoft Teams"
                detectedTitle = "Microsoft Teams Meeting"
                hasCallControls = true
                break
            } else if bid == "us.zoom.xos" {
                detectedApp = "Zoom"
                detectedTitle = "Zoom Meeting"
                hasCallControls = true
                break
            } else if bid == "com.apple.facetime" {
                detectedApp = "FaceTime"
                detectedTitle = "FaceTime"
                hasCallControls = true
                break
            }
        }
    }

    let payload: [String: Any] = [
        "microphone_in_use": micRunning,
        "system_audio_active": micRunning && (detectedApp != nil),
        "has_call_controls_ui": hasCallControls,
        "detected_meeting_app": jsonNullable(detectedApp),
        "detected_meeting_title": jsonNullable(detectedTitle)
    ]
    JSONOut.emit(payload)
}

func handleRecordDictation(outPath: String) {
    let stopLock = NSLock()
    var shouldStop = false
    func requestStop() {
        stopLock.lock()
        shouldStop = true
        stopLock.unlock()
    }
    func isStopping() -> Bool {
        stopLock.lock()
        defer { stopLock.unlock() }
        return shouldStop
    }

    DispatchQueue.global(qos: .utility).async {
        while true {
            let data = FileHandle.standardInput.availableData
            if data.isEmpty {
                requestStop()
                break
            }
            if let text = String(data: data, encoding: .utf8), text.lowercased().contains("stop") {
                requestStop()
                break
            }
        }
    }

    signal(SIGTERM, SIG_IGN)
    let sigTermSrc = DispatchSource.makeSignalSource(signal: SIGTERM, queue: DispatchQueue.global())
    sigTermSrc.setEventHandler { requestStop() }
    sigTermSrc.resume()

    let micRec = ContinuousMicRecorder()
    micRec.start()
    let startTime = Date()

    // Max safety limit: 5 minutes continuous dictation
    while !isStopping() && Date().timeIntervalSince(startTime) < 300.0 {
        Thread.sleep(forTimeInterval: 0.15)
        let elapsedMs: Int = Int(Date().timeIntervalSince(startTime) * 1000.0)
        let rms: Double = micRec.currentRms()
        let lvl: [String: Any] = [
            "type": "level",
            "rms": rms,
            "elapsed_ms": elapsedMs,
            "mic_active": micRec.isRunning,
            "error": jsonNullable(micRec.errorMessage)
        ]
        JSONOut.emit(lvl)
    }

    micRec.stop()
    sigTermSrc.cancel()
    let samples = micRec.drainSamples()
    let sampleCount: Int = samples.count
    let durMs: Int = Int(Date().timeIntervalSince(startTime) * 1000.0)
    let ok = sampleCount > 1600 && (try? writeMono16kWav(samples: samples, to: outPath)) != nil
    let donePayload: [String: Any] = [
        "type": "done",
        "ok": ok,
        "wav_path": jsonNullable(ok ? outPath : nil),
        "samples": sampleCount,
        "duration_ms": durMs,
        "error": jsonNullable(micRec.errorMessage)
    ]
    JSONOut.emit(donePayload)
}

func handlePasteText() {
    let inputData = FileHandle.standardInput.readDataToEndOfFile()
    let text = String(data: inputData, encoding: .utf8) ?? ""
    if text.isEmpty {
        let res: [String: Any] = ["ok": false]
        JSONOut.emit(res)
        return
    }
    let pb = NSPasteboard.general
    let previousString: String? = pb.string(forType: .string)

    pb.clearContents()
    pb.setString(text, forType: .string)
    Thread.sleep(forTimeInterval: 0.04)

    var posted = false
    if let src = CGEventSource(stateID: .hidSystemState),
       let keyDown = CGEvent(keyboardEventSource: src, virtualKey: 9, keyDown: true),
       let keyUp = CGEvent(keyboardEventSource: src, virtualKey: 9, keyDown: false) {
        keyDown.flags = .maskCommand
        keyUp.flags = .maskCommand
        keyDown.post(tap: .cghidEventTap)
        keyUp.post(tap: .cghidEventTap)
        posted = true
    }

    // Brief delay so target app reads pasted string before restoring user's original clipboard
    Thread.sleep(forTimeInterval: 0.18)
    if let prev = previousString {
        pb.clearContents()
        pb.setString(prev, forType: .string)
    }

    let res: [String: Any] = ["ok": posted]
    JSONOut.emit(res)
}

func handleRecordMic(outPath: String, seconds: Double) {
    let samples = recordMicrophoneSamples(durationSecs: seconds)
    let sampleCount: Int = samples.count
    let ok = (try? writeMono16kWav(samples: samples, to: outPath)) != nil && sampleCount > 0
    let payload: [String: Any] = ["ok": ok, "wav_path": outPath, "samples": sampleCount]
    JSONOut.emit(payload)
}

// Entrypoint CLI Router
let args = CommandLine.arguments
let cmd = args.count > 1 ? args[1] : "permissions"

switch cmd {
case "permissions":
    handlePermissions()
case "open-settings":
    handleOpenSettings(args.count > 2 ? args[2] : "screen")
case "request-permission":
    handleRequestPermission(args.count > 2 ? args[2] : "screen")
case "active-window":
    handleActiveWindow()
case "audio-env":
    handleAudioEnvironment()
case "capture-window-ocr":
    let includeImg = args.contains("--image")
    handleCaptureWindowOCR(includeImage: includeImg)
case "clipboard":
    handleClipboard()
case "calendar":
    handleCalendar()
case "record-dual-audio":
    let outDir = args.count > 2 ? args[2] : "/tmp/backtext-audio"
    let secs = args.count > 3 ? (Double(args[3]) ?? 8.0) : 8.0
    handleRecordDualAudio(outDir: outDir, seconds: secs)
case "stream-dual-audio":
    let outDir = args.count > 2 ? args[2] : "/tmp/backtext-audio"
    let secs = args.count > 3 ? (Double(args[3]) ?? 4.5) : 4.5
    handleStreamDualAudio(outDir: outDir, chunkSecs: secs)
case "record-dictation":
    let outWav = args.count > 2 ? args[2] : "/tmp/backtext-dictation.wav"
    handleRecordDictation(outPath: outWav)
case "paste-text":
    handlePasteText()
case "record-mic":
    let outWav = args.count > 2 ? args[2] : "/tmp/backtext-self.wav"
    let secs = args.count > 3 ? (Double(args[3]) ?? 6.0) : 6.0
    handleRecordMic(outPath: outWav, seconds: secs)
default:
    JSONOut.emit(["error": "Unknown subcommand: \(cmd)"])
}
