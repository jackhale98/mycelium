import UIKit
import UniformTypeIdentifiers
import WebKit

/// Native iOS folder picker that presents UIDocumentPickerViewController.
/// Called from JavaScript via window.webkit.messageHandlers.
class FolderPickerHandler: NSObject, UIDocumentPickerDelegate {
    static let shared = FolderPickerHandler()

    private var completionHandler: ((String?) -> Void)?
    private weak var presentingVC: UIViewController?

    /// Present the native iOS folder picker
    func pickFolder(from viewController: UIViewController, completion: @escaping (String?) -> Void) {
        self.completionHandler = completion
        self.presentingVC = viewController

        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [UTType.folder])
        picker.delegate = self
        picker.allowsMultipleSelection = false
        picker.modalPresentationStyle = .formSheet

        viewController.present(picker, animated: true)
    }

    // MARK: - UIDocumentPickerDelegate

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let url = urls.first else {
            completionHandler?(nil)
            completionHandler = nil
            return
        }

        // Get security-scoped access to the folder
        let accessing = url.startAccessingSecurityScopedResource()

        // Create a bookmark for persistent access across app launches
        if let bookmarkData = try? url.bookmarkData(
            options: .minimalBookmark,
            includingResourceValuesForKeys: nil,
            relativeTo: nil
        ) {
            UserDefaults.standard.set(bookmarkData, forKey: "mycelium_vault_bookmark")
        }

        completionHandler?(url.path)
        completionHandler = nil

        // Don't stop accessing — the app needs ongoing access to read files
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        completionHandler?(nil)
        completionHandler = nil
    }

    /// Try to restore access from a saved bookmark
    static func restoreBookmarkAccess() -> String? {
        guard let bookmarkData = UserDefaults.standard.data(forKey: "mycelium_vault_bookmark") else {
            return nil
        }

        var isStale = false
        guard let url = try? URL(
            resolvingBookmarkData: bookmarkData,
            options: [],
            relativeTo: nil,
            bookmarkDataIsStale: &isStale
        ) else {
            return nil
        }

        if isStale {
            // Re-create bookmark
            if let newBookmark = try? url.bookmarkData(
                options: .minimalBookmark,
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            ) {
                UserDefaults.standard.set(newBookmark, forKey: "mycelium_vault_bookmark")
            }
        }

        _ = url.startAccessingSecurityScopedResource()
        return url.path
    }
}
