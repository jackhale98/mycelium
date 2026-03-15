import UIKit
import UniformTypeIdentifiers
import Tauri

class FolderPickerPlugin: Plugin {
    private var pendingInvoke: Invoke?

    @objc public func pickFolder(_ invoke: Invoke) {
        self.pendingInvoke = invoke

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }

            let picker = UIDocumentPickerViewController(
                forOpeningContentTypes: [UTType.folder]
            )
            picker.delegate = self
            picker.allowsMultipleSelection = false
            picker.modalPresentationStyle = .formSheet

            if let viewController = self.manager.viewController {
                viewController.present(picker, animated: true)
            } else {
                invoke.reject("No view controller available")
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

        // Start accessing the security-scoped resource
        let accessing = url.startAccessingSecurityScopedResource()

        // Store a bookmark for persistent access across app launches
        do {
            let bookmarkData = try url.bookmarkData(
                options: .minimalBookmark,
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )

            // Save bookmark to UserDefaults for later access
            UserDefaults.standard.set(bookmarkData, forKey: "vault_bookmark")
            UserDefaults.standard.set(url.path, forKey: "vault_path")

            pendingInvoke?.resolve(["path": url.path])
        } catch {
            pendingInvoke?.resolve(["path": url.path])
        }

        if accessing {
            // Note: we don't stop accessing here because the app needs ongoing access
            // The bookmark allows re-accessing after app restart
        }

        pendingInvoke = nil
    }

    public func documentPickerWasCancelled(
        _ controller: UIDocumentPickerViewController
    ) {
        pendingInvoke?.resolve(["path": NSNull()])
        pendingInvoke = nil
    }
}
