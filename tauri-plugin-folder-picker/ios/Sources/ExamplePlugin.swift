import SwiftRs
import Tauri
import UIKit
import WebKit
import UniformTypeIdentifiers

class FolderPickerPlugin: Plugin {
    private var pendingInvoke: Invoke?

    @objc public func pickFolder(_ invoke: Invoke) throws {
        self.pendingInvoke = invoke

        DispatchQueue.main.async { [weak self] in
            guard let self = self else {
                invoke.reject("Plugin deallocated")
                return
            }

            let picker = UIDocumentPickerViewController(
                forOpeningContentTypes: [UTType.folder]
            )
            picker.delegate = self
            picker.allowsMultipleSelection = false
            picker.modalPresentationStyle = .formSheet

            if let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
               let rootVC = scene.windows.first?.rootViewController {
                var topVC = rootVC
                while let presented = topVC.presentedViewController {
                    topVC = presented
                }
                topVC.present(picker, animated: true)
            } else {
                invoke.reject("No view controller available")
                self.pendingInvoke = nil
            }
        }
    }
}

extension FolderPickerPlugin: UIDocumentPickerDelegate {
    public func documentPicker(
        _ controller: UIDocumentPickerViewController,
        didPickDocumentsAt urls: [URL]
    ) {
        guard let url = urls.first else {
            pendingInvoke?.reject("No folder selected")
            pendingInvoke = nil
            return
        }

        let accessing = url.startAccessingSecurityScopedResource()

        do {
            let bookmarkData = try url.bookmarkData(
                options: .minimalBookmark,
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )
            UserDefaults.standard.set(bookmarkData, forKey: "mycelium_vault_bookmark")
            UserDefaults.standard.set(url.path, forKey: "mycelium_vault_path")
        } catch {
            // Still return the path even if bookmark fails
        }

        pendingInvoke?.resolve(["path": url.path])
        pendingInvoke = nil
    }

    public func documentPickerWasCancelled(
        _ controller: UIDocumentPickerViewController
    ) {
        pendingInvoke?.resolve(["path": NSNull()])
        pendingInvoke = nil
    }
}

@_cdecl("init_plugin_folder_picker")
func initPlugin() -> Plugin {
    return FolderPickerPlugin()
}
