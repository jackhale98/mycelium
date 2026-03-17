import SwiftRs
import Tauri
import UIKit
import WebKit
import UniformTypeIdentifiers

class FolderPickerPlugin: Plugin {
    private var pendingInvoke: Invoke?
    /// Track the actively accessed security-scoped URL so we can stop/restart
    private var activeSecurityScopedURL: URL?

    override init() {
        super.init()
        // Auto-restore bookmark access on plugin init (app launch)
        _ = Self.restoreBookmarkAccess()
    }

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

    /// Restore security-scoped access from a stored bookmark.
    /// Called automatically on init and can be called explicitly via command.
    @objc public func restoreAccess(_ invoke: Invoke) throws {
        if let path = Self.restoreBookmarkAccess() {
            invoke.resolve(["path": path])
        } else {
            invoke.resolve(["path": NSNull()])
        }
    }

    /// Static helper: resolve stored bookmark and activate security-scoped access.
    /// Returns the path if successful, nil otherwise.
    @discardableResult
    static func restoreBookmarkAccess() -> String? {
        guard let bookmarkData = UserDefaults.standard.data(forKey: "mycelium_vault_bookmark") else {
            return nil
        }

        do {
            var isStale = false
            let url = try URL(
                resolvingBookmarkData: bookmarkData,
                options: [],
                relativeTo: nil,
                bookmarkDataIsStale: &isStale
            )

            // Activate security-scoped access for the entire directory (includes subdirectories)
            let accessing = url.startAccessingSecurityScopedResource()
            NSLog("[Mycelium] Restored bookmark access for: %@ (accessing=%d, stale=%d)", url.path, accessing ? 1 : 0, isStale ? 1 : 0)

            if isStale {
                // Re-save the bookmark if stale
                if let newBookmark = try? url.bookmarkData(
                    options: .minimalBookmark,
                    includingResourceValuesForKeys: nil,
                    relativeTo: nil
                ) {
                    UserDefaults.standard.set(newBookmark, forKey: "mycelium_vault_bookmark")
                    NSLog("[Mycelium] Re-saved stale bookmark")
                }
            }

            return url.path
        } catch {
            NSLog("[Mycelium] Failed to restore bookmark: %@", error.localizedDescription)
            return nil
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

        // Stop previous security-scoped access if any
        activeSecurityScopedURL?.stopAccessingSecurityScopedResource()

        let accessing = url.startAccessingSecurityScopedResource()
        NSLog("[Mycelium] Picked folder: %@ (accessing=%d)", url.path, accessing ? 1 : 0)

        if accessing {
            activeSecurityScopedURL = url
        }

        // Enumerate subdirectories to verify access
        if let enumerator = FileManager.default.enumerator(at: url, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) {
            var dirCount = 0
            var fileCount = 0
            while let item = enumerator.nextObject() as? URL {
                let isDir = (try? item.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory ?? false
                if isDir { dirCount += 1 } else { fileCount += 1 }
            }
            NSLog("[Mycelium] Vault contents: %d dirs, %d files", dirCount, fileCount)
        }

        do {
            let bookmarkData = try url.bookmarkData(
                options: .minimalBookmark,
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )
            UserDefaults.standard.set(bookmarkData, forKey: "mycelium_vault_bookmark")
            UserDefaults.standard.set(url.path, forKey: "mycelium_vault_path")
            NSLog("[Mycelium] Saved bookmark for: %@", url.path)
        } catch {
            NSLog("[Mycelium] Bookmark save failed: %@", error.localizedDescription)
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
