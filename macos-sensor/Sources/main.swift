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
        } else {
            print("{}")
        }
    }
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
    if bundleID == "com.apple.Safari" {
        script = "tell application \"Safari\" to if (count of documents) > 0 then return URL of front document"
    } else if ["com.google.Chrome", "company.thebrowser.Browser", "com.brave.Browser", "com.microsoft.edgemac"].contains(bundleID) {
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

    JSONOut.emit([
        "platform": "macos",
        "screen_capture_granted": sck,
        "microphone_granted": mic,
        "accessibility_granted": ax,
        "calendar_granted": cal,
        "idle_seconds": Int(idleSecs),
        "screen_locked": screenLocked
    ])
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

    JSONOut.emit([
        "window_id": Int(pid),
        "app_bundle_id": bundleID,
        "app_name": appName,
        "window_title": windowTitle,
        "browser_url": browserURL as Any,
        "visible_text": secureFocused || isPrivate ? "" : (joinedText.isEmpty ? "\(appName) — \(windowTitle)" : joinedText),
        "is_private_browsing": isPrivate,
        "is_secure_field_focused": secureFocused,
        "detected_screen_share_presenter": presenter as Any,
        "detected_active_speaker_tile": activeSpeaker as Any,
        "mic_running": isDefaultMicRunning()
    ])
}

@available(macOS 14.0, *)
func captureFrontmostSCImage() async -> CGImage? {
    guard CGPreflightScreenCaptureAccess() else { return nil }
    do {
        let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        guard let frontPID = NSWorkspace.shared.frontmostApplication?.processIdentifier else { return nil }
        guard let targetWin = content.windows.first(where: {
            $0.owningApplication?.processID == frontPID && $0.frame.width > 120 && $0.frame.height > 120
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

    JSONOut.emit([
        "ok": true,
        "dhash_u64": dhash,
        "dhash_hex": String(format: "%016llx", dhash),
        "ocr_text": ocr,
        "image_data_url": dataURL as Any
    ])
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
    JSONOut.emit([
        "change_count": pb.changeCount,
        "source_app": front,
        "text": text,
        "is_concealed": concealed
    ])
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
            "conference_url": confURL as Any,
            "attendees": attendees
        ]
    }
    JSONOut.emit(["authorized": true, "events": mapped])
}

func writeMono16kWav(samples: [Int16], to path: String) throws {
    var data = Data()
    let sampleRate: UInt32 = 16000
    let numChannels: UInt16 = 1
    let bitsPerSample: UInt16 = 16
    let dataSize = UInt32(samples.count * 2)
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
    samples.withUnsafeBufferPointer { buf in
        data.append(UnsafeBufferPointer(start: UnsafeRawPointer(buf.baseAddress!).assumingMemoryBound(to: UInt8.self), count: samples.count * 2))
    }
    try data.write(to: URL(fileURLWithPath: path))
}

func recordMicrophoneSamples(durationSecs: Double) -> [Int16] {
    let engine = AVAudioEngine()
    let input = engine.inputNode
    let inputFmt = input.outputFormat(forBus: 0)
    guard let targetFmt = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16000, channels: 1, interleaved: true),
          let converter = AVAudioConverter(from: inputFmt, to: targetFmt) else {
        return []
    }

    var collected: [Int16] = []
    let lock = NSLock()

    input.installTap(onBus: 0, bufferSize: 2048, format: inputFmt) { buffer, _ in
        let ratio = 16000.0 / max(1.0, inputFmt.sampleRate)
        let outFrames = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 64
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
            return buffer
        }
        if err == nil, let ptr = outBuf.int16ChannelData?[0] {
            let count = Int(outBuf.frameLength)
            lock.lock()
            collected.append(contentsOf: UnsafeBufferPointer(start: ptr, count: count))
            lock.unlock()
        }
    }

    do {
        try engine.start()
        Thread.sleep(forTimeInterval: durationSecs)
        input.removeTap(onBus: 0)
        engine.stop()
    } catch {
        return []
    }
    return collected
}

@available(macOS 13.0, *)
final class SystemAudioCollector: NSObject, SCStreamOutput {
    private let lock = NSLock()
    private var samples: [Int16] = []

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio, sampleBuffer.isValid else { return }
        guard let blockBuf = CMSampleBufferGetDataBuffer(sampleBuffer) else { return }
        var totalLen = 0
        var dataPtr: UnsafeMutablePointer<Int8>?
        guard CMBlockBufferGetDataPointer(blockBuf, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: &totalLen, dataPointerOut: &dataPtr) == kCMBlockBufferNoErr,
              let ptr = dataPtr else { return }

        // ScreenCaptureKit delivers 32-bit float PCM at configured 48kHz/16kHz -> downsample cleanly to 16kHz mono Int16
        let floatCount = totalLen / MemoryLayout<Float>.size
        let floatPtr = UnsafeRawPointer(ptr).assumingMemoryBound(to: Float.self)
        var chunk: [Int16] = []
        chunk.reserveCapacity(floatCount / 3)
        var i = 0
        while i < floatCount {
            let clamped = max(-1.0, min(1.0, floatPtr[i]))
            chunk.append(Int16(clamped * 32767.0))
            i += 3 // 48kHz -> 16kHz decimation
        }
        lock.lock()
        samples.append(contentsOf: chunk)
        lock.unlock()
    }

    func snapshot() -> [Int16] {
        lock.lock()
        defer { lock.unlock() }
        return samples
    }
}

func handleRecordDualAudio(outDir: String, seconds: Double) {
    try? FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true)
    let micPath = (outDir as NSString).appendingPathComponent("mic.wav")
    let sysPath = (outDir as NSString).appendingPathComponent("system.wav")

    var micSamples: [Int16] = []
    var sysSamples: [Int16] = []
    let group = DispatchGroup()

    // Channel 0: Own Microphone
    group.enter()
    DispatchQueue.global(qos: .userInitiated).async {
        micSamples = recordMicrophoneSamples(durationSecs: seconds)
        group.leave()
    }

    // Channel 1: ScreenCaptureKit System Audio (Bot-free!)
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
                // Non-fatal fallback if audio tap unavailable
            }
            group.leave()
        }
    }

    group.wait()
    let micOk = (try? writeMono16kWav(samples: micSamples, to: micPath)) != nil && !micSamples.isEmpty
    let sysOk = (try? writeMono16kWav(samples: sysSamples, to: sysPath)) != nil && !sysSamples.isEmpty
    JSONOut.emit([
        "mic_wav_path": micOk ? micPath : NSNull(),
        "mic_samples": micSamples.count,
        "system_wav_path": sysOk ? sysPath : NSNull(),
        "system_samples": sysSamples.count
    ])
}

func handleRecordMic(outPath: String, seconds: Double) {
    let samples = recordMicrophoneSamples(durationSecs: seconds)
    let ok = (try? writeMono16kWav(samples: samples, to: outPath)) != nil && !samples.isEmpty
    JSONOut.emit(["ok": ok, "wav_path": outPath, "samples": samples.count])
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
case "record-mic":
    let outWav = args.count > 2 ? args[2] : "/tmp/backtext-self.wav"
    let secs = args.count > 3 ? (Double(args[3]) ?? 6.0) : 6.0
    handleRecordMic(outPath: outWav, seconds: secs)
default:
    JSONOut.emit(["error": "Unknown subcommand: \(cmd)"])
}
