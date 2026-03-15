import UIKit
import WebKit

/// WKScriptMessageHandler that bridges JavaScript calls to the native folder picker.
/// Register this on the WKWebView's userContentController to enable
/// window.webkit.messageHandlers.folderPicker.postMessage("pick") from JS.
class FolderPickerBridge: NSObject, WKScriptMessageHandler {
    weak var webView: WKWebView?

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.name == "folderPicker" else { return }
        guard let action = message.body as? String else { return }

        if action == "pick" {
            pickFolder()
        } else if action == "restore" {
            restoreAccess()
        }
    }

    private func pickFolder() {
        guard let webView = webView,
              let viewController = webView.window?.rootViewController else { return }

        FolderPickerHandler.shared.pickFolder(from: viewController) { [weak self] path in
            if let path = path {
                let js = "window.__myceliumFolderPickerCallback && window.__myceliumFolderPickerCallback('\(path.replacingOccurrences(of: "'", with: "\\'"))')"
                DispatchQueue.main.async {
                    self?.webView?.evaluateJavaScript(js, completionHandler: nil)
                }
            } else {
                let js = "window.__myceliumFolderPickerCallback && window.__myceliumFolderPickerCallback(null)"
                DispatchQueue.main.async {
                    self?.webView?.evaluateJavaScript(js, completionHandler: nil)
                }
            }
        }
    }

    private func restoreAccess() {
        if let path = FolderPickerHandler.restoreBookmarkAccess() {
            let js = "window.__myceliumFolderRestoreCallback && window.__myceliumFolderRestoreCallback('\(path.replacingOccurrences(of: "'", with: "\\'"))')"
            DispatchQueue.main.async { [weak self] in
                self?.webView?.evaluateJavaScript(js, completionHandler: nil)
            }
        }
    }
}
