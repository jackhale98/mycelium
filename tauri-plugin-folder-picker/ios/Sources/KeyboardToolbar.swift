import UIKit
import WebKit

/// Encode `value` as a JavaScript string literal (quotes included) so it can be
/// interpolated into a script passed to `evaluateJavaScript` without breaking or
/// injecting into it.
private func jsString(_ value: String) -> String {
    guard let data = try? JSONSerialization.data(withJSONObject: [value], options: []),
          let json = String(data: data, encoding: .utf8),
          json.count > 2 else {
        return "\"\""
    }
    // Drop the array brackets JSONSerialization needs for a top-level value.
    return String(json.dropFirst().dropLast())
        .replacingOccurrences(of: "<", with: "\\u003C")
        .replacingOccurrences(of: ">", with: "\\u003E")
        .replacingOccurrences(of: "\u{2028}", with: "\\u2028")
        .replacingOccurrences(of: "\u{2029}", with: "\\u2029")
}

/// Key the toolbar is attached to its web view under.
private var toolbarAssociationKey: UInt8 = 0

/// Name of the script message the web app posts when focus enters or leaves
/// the note editor.
let editorFocusMessageName = "myceliumEditorFocus"

/// Native iOS keyboard toolbar that sits above the software keyboard while the
/// note editor has focus.
///
/// The toolbar is the web view's `inputAccessoryView`. Since iOS 13 (WebKit
/// r246229) the view that actually becomes first responder inside a
/// `WKWebView` asks the web view for its input accessory, so overriding that
/// public `UIResponder` property on the web view is the supported way to supply
/// one. Tauri creates the web view itself, so the override is added to its
/// class at runtime instead of by declaring a subclass; no private WebKit class
/// is named or touched.
///
/// The toolbar is offered only while the editor is focused. Every button acts
/// on the editor, so showing it over the search field, a modal or quick capture
/// put commands on screen that edited a note the user was not looking at. Every
/// other field gets no accessory at all, which also drops WebKit's
/// previous/next/done bar — the look of a native text field.
class KeyboardToolbar: UIView {
    private weak var webView: WKWebView?

    /// Whether focus is inside the note editor, as last reported by the page.
    private(set) var editorFocused = false

    /// Create and install the toolbar on a WKWebView. Installing twice is a no-op.
    static func install(on webView: WKWebView) {
        if objc_getAssociatedObject(webView, &toolbarAssociationKey) is KeyboardToolbar {
            return
        }
        let toolbar = KeyboardToolbar(webView: webView)
        // The web view owns the toolbar; the toolbar only holds it weakly.
        objc_setAssociatedObject(webView, &toolbarAssociationKey, toolbar, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        overrideInputAccessoryView(of: webView)

        // A script message, not a plugin command, carries focus changes: it
        // travels on the same channel as WebKit's own "element focused"
        // notification and arrives ahead of it, so the keyboard comes up with
        // the right accessory instead of growing one a moment later.
        let controller = webView.configuration.userContentController
        controller.removeScriptMessageHandler(forName: editorFocusMessageName)
        controller.add(EditorFocusHandler(toolbar: toolbar), name: editorFocusMessageName)
        NSLog("[Mycelium] Keyboard toolbar installed")
    }

    /// The toolbar installed on `webView`, if any.
    static func installed(on webView: WKWebView) -> KeyboardToolbar? {
        objc_getAssociatedObject(webView, &toolbarAssociationKey) as? KeyboardToolbar
    }

    private init(webView: WKWebView) {
        self.webView = webView
        super.init(frame: CGRect(x: 0, y: 0, width: UIScreen.main.bounds.width, height: 44))
        autoresizingMask = .flexibleWidth
        backgroundColor = .secondarySystemBackground
        setupUI()
    }

    required init?(coder: NSCoder) { fatalError() }

    /// Record whether the editor has focus, and swap the accessory if a
    /// keyboard is already showing.
    func setEditorFocused(_ focused: Bool) {
        guard focused != editorFocused else { return }
        editorFocused = focused
        // UIKit only re-reads inputAccessoryView when the first responder asks
        // it to. Focus moving between fields inside the page keeps the same
        // first responder, so it has to be told.
        if let webView = webView, let responder = Self.firstResponder(in: webView) {
            responder.reloadInputViews()
        }
    }

    // MARK: - UI Setup

    private func setupUI() {
        // Top border
        let border = UIView()
        border.backgroundColor = .separator
        border.frame = CGRect(x: 0, y: 0, width: bounds.width, height: 0.5)
        border.autoresizingMask = .flexibleWidth
        addSubview(border)

        // Dismiss button, pinned to the trailing edge so it is always reachable
        // however far the buttons are scrolled.
        let dismissWidth: CGFloat = 44
        let dismiss = UIButton(type: .system)
        dismiss.frame = CGRect(x: bounds.width - dismissWidth, y: 0.5, width: dismissWidth, height: 43.5)
        dismiss.autoresizingMask = [.flexibleLeftMargin, .flexibleHeight]
        dismiss.setImage(UIImage(systemName: "keyboard.chevron.compact.down"), for: .normal)
        dismiss.accessibilityLabel = "Hide keyboard"
        dismiss.addTarget(self, action: #selector(dismissTapped), for: .touchUpInside)
        addSubview(dismiss)

        let dismissSeparator = UIView(frame: CGRect(x: bounds.width - dismissWidth, y: 8, width: 0.5, height: 28))
        dismissSeparator.backgroundColor = .separator
        dismissSeparator.autoresizingMask = [.flexibleLeftMargin]
        addSubview(dismissSeparator)

        // Scroll view for horizontal scrolling
        let scroll = UIScrollView()
        scroll.frame = CGRect(x: 0, y: 0.5, width: bounds.width - dismissWidth, height: 43.5)
        scroll.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        scroll.showsHorizontalScrollIndicator = false
        scroll.showsVerticalScrollIndicator = false
        addSubview(scroll)

        // Button definitions: (label, action, color, bold, width)
        let buttons: [(String, String, UIColor?, Bool, CGFloat)] = [
            ("Link", "link", .systemGreen, true, 44),
            ("|", "", nil, false, 1),
            ("H", "heading", nil, true, 36),
            ("ID", "makeNode", .systemPurple, true, 32),
            ("TODO", "todo", .systemRed, true, 48),
            ("[#]", "priority", .systemOrange, true, 38),
            ("Tag", "tag", .systemTeal, true, 38),
            ("DL", "deadline", .systemRed, false, 32),
            ("SC", "scheduled", .systemBlue, false, 32),
            ("|", "", nil, false, 1),
            ("B", "bold", nil, true, 32),
            ("I", "italic", nil, false, 32),
            ("U", "underline", nil, false, 32),
            ("S", "strike", nil, false, 32),
            ("<>", "code", nil, false, 32),
            ("=", "verbatim", nil, false, 28),
            ("|", "", nil, false, 1),
            ("•", "list", nil, false, 28),
            ("☐", "checkbox", nil, false, 32),
            ("⊞", "table", nil, false, 32),
            ("{ }", "srcblock", nil, false, 38),
            ("❝", "quote", nil, false, 28),
            ("Date", "timestamp", nil, false, 38),
        ]

        var x: CGFloat = 8
        let btnHeight: CGFloat = 36
        let yOffset: CGFloat = (43.5 - btnHeight) / 2

        for (label, action, color, bold, width) in buttons {
            if label == "|" {
                let sep = UIView(frame: CGRect(x: x, y: 8, width: 1, height: 28))
                sep.backgroundColor = .separator
                scroll.addSubview(sep)
                x += 5
                continue
            }

            let btn = UIButton(type: .system)
            btn.frame = CGRect(x: x, y: yOffset, width: width, height: btnHeight)
            btn.setTitle(label, for: .normal)
            btn.accessibilityIdentifier = action
            btn.accessibilityLabel = Self.accessibilityLabels[action] ?? label

            if let color = color {
                btn.setTitleColor(color, for: .normal)
            }

            btn.titleLabel?.font = bold
                ? UIFont.boldSystemFont(ofSize: 13)
                : UIFont.systemFont(ofSize: 12)

            btn.addTarget(self, action: #selector(buttonTapped(_:)), for: .touchUpInside)
            scroll.addSubview(btn)
            x += width + 2
        }

        scroll.contentSize = CGSize(width: x + 8, height: 43.5)
    }

    /// Spoken names for the buttons, whose titles are mostly abbreviations.
    private static let accessibilityLabels: [String: String] = [
        "link": "Insert link",
        "heading": "Heading",
        "makeNode": "Make heading a node",
        "todo": "TODO state",
        "priority": "Priority",
        "tag": "File tags",
        "deadline": "Deadline",
        "scheduled": "Scheduled",
        "bold": "Bold",
        "italic": "Italic",
        "underline": "Underline",
        "strike": "Strikethrough",
        "code": "Code",
        "verbatim": "Verbatim",
        "list": "List item",
        "checkbox": "Checkbox",
        "table": "Table",
        "srcblock": "Source block",
        "quote": "Quote",
        "timestamp": "Insert date",
    ]

    // MARK: - Button Tap Handler

    @objc private func dismissTapped() {
        // Blurring the editor is what tells WebKit the keyboard can go; ending
        // editing natively would leave the page believing it still has focus.
        webView?.evaluateJavaScript("document.activeElement && document.activeElement.blur()", completionHandler: nil)
    }

    @objc private func buttonTapped(_ sender: UIButton) {
        guard let action = sender.accessibilityIdentifier, !action.isEmpty else { return }

        switch action {
        case "todo":
            showTodoPicker(from: sender)
        case "heading":
            showHeadingPicker(from: sender)
        case "priority":
            showPriorityPicker(from: sender)
        case "tag":
            showTagPicker(from: sender)
        case "table":
            showTablePicker(from: sender)
        case "deadline":
            showDatePicker(for: "deadline", from: sender)
        case "scheduled":
            showDatePicker(for: "scheduled", from: sender)
        default:
            let js = "window.__myceliumToolbar && window.__myceliumToolbar.\(action)()"
            webView?.evaluateJavaScript(js, completionHandler: nil)
        }
    }

    // MARK: - Pickers

    private func showTodoPicker(from sender: UIButton) {
        // Fetch user-configured keywords from JS orgConfig store
        let js = "JSON.stringify({ todo: window.__myceliumOrgConfig?.todoKeywords ?? ['TODO'], done: window.__myceliumOrgConfig?.doneKeywords ?? ['DONE'] })"
        webView?.evaluateJavaScript(js) { [weak self] result, _ in
            guard let self = self else { return }
            var todoKw = ["TODO"]
            var doneKw = ["DONE"]
            if let jsonStr = result as? String,
               let data = try? JSONSerialization.jsonObject(with: Data(jsonStr.utf8)) as? [String: [String]] {
                todoKw = data["todo"] ?? todoKw
                doneKw = data["done"] ?? doneKw
            }

            DispatchQueue.main.async {
                let alert = UIAlertController(title: "Set TODO State", message: nil, preferredStyle: .actionSheet)
                alert.addAction(UIAlertAction(title: "None", style: .default) { _ in
                    self.webView?.evaluateJavaScript("window.__myceliumToolbar?.todoSet(null)", completionHandler: nil)
                })
                for kw in todoKw {
                    alert.addAction(UIAlertAction(title: kw, style: .default) { _ in
                        self.webView?.evaluateJavaScript("window.__myceliumToolbar?.todoSet(\(jsString(kw)))", completionHandler: nil)
                    })
                }
                for kw in doneKw {
                    alert.addAction(UIAlertAction(title: "✓ \(kw)", style: .default) { _ in
                        self.webView?.evaluateJavaScript("window.__myceliumToolbar?.todoSet(\(jsString(kw)))", completionHandler: nil)
                    })
                }
                alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
                if let popover = alert.popoverPresentationController {
                    popover.sourceView = sender
                    popover.sourceRect = sender.bounds
                }
                self.presentAlert(alert)
            }
        }
    }

    private func showHeadingPicker(from sender: UIButton) {
        let alert = UIAlertController(title: "Insert Heading", message: nil, preferredStyle: .actionSheet)

        alert.addAction(UIAlertAction(title: "Same level (auto)", style: .default) { [weak self] _ in
            self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.heading()", completionHandler: nil)
        })
        for level in 1...4 {
            let stars = String(repeating: "*", count: level)
            alert.addAction(UIAlertAction(title: "\(stars) Heading \(level)", style: .default) { [weak self] _ in
                self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.headingLevel(\(level))", completionHandler: nil)
            })
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))

        if let popover = alert.popoverPresentationController {
            popover.sourceView = sender
            popover.sourceRect = sender.bounds
        }
        presentAlert(alert)
    }

    private func showPriorityPicker(from sender: UIButton) {
        let js = "JSON.stringify(window.__myceliumOrgConfig?.priorities ?? ['A','B','C'])"
        webView?.evaluateJavaScript(js) { [weak self] result, _ in
            guard let self = self else { return }
            var priorities = ["A", "B", "C"]
            if let jsonStr = result as? String,
               let data = try? JSONSerialization.jsonObject(with: Data(jsonStr.utf8)) as? [String] {
                priorities = data
            }

            DispatchQueue.main.async {
                let alert = UIAlertController(title: "Set Priority", message: nil, preferredStyle: .actionSheet)
                alert.addAction(UIAlertAction(title: "None", style: .default) { _ in
                    self.webView?.evaluateJavaScript("window.__myceliumToolbar?.prioritySet(null)", completionHandler: nil)
                })
                for p in priorities {
                    alert.addAction(UIAlertAction(title: "[#\(p)]", style: .default) { _ in
                        self.webView?.evaluateJavaScript("window.__myceliumToolbar?.prioritySet(\(jsString(p)))", completionHandler: nil)
                    })
                }
                alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
                if let popover = alert.popoverPresentationController {
                    popover.sourceView = sender
                    popover.sourceRect = sender.bounds
                }
                self.presentAlert(alert)
            }
        }
    }

    private func showTablePicker(from sender: UIButton) {
        let alert = UIAlertController(title: "Insert Table", message: nil, preferredStyle: .actionSheet)
        let sizes = ["2 × 2", "3 × 3", "4 × 3", "5 × 3", "2 × 4", "3 × 5"]
        let dims = [(2,2), (3,3), (4,3), (5,3), (2,4), (3,5)] // (cols, rows)
        for (i, label) in sizes.enumerated() {
            let (cols, rows) = dims[i]
            alert.addAction(UIAlertAction(title: "\(label) (\(cols) cols × \(rows) rows)", style: .default) { [weak self] _ in
                self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.tableSize(\(rows), \(cols))", completionHandler: nil)
            })
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        if let popover = alert.popoverPresentationController {
            popover.sourceView = sender
            popover.sourceRect = sender.bounds
        }
        presentAlert(alert)
    }

    private func showTagPicker(from sender: UIButton) {
        // Get current file tags and all vault tags
        let jsFiletags = "window.__myceliumToolbar?.getFiletags?.() ?? '[]'"
        let jsAllTags = "JSON.stringify((window.__myceliumVaultTags ?? []).map(t => t.tag || t))"

        webView?.evaluateJavaScript(jsFiletags) { [weak self] fileResult, _ in
            guard let self = self else { return }
            self.webView?.evaluateJavaScript(jsAllTags) { [weak self] allResult, _ in
                guard let self = self else { return }

                var currentTags: [String] = []
                var allTags: [String] = []

                if let jsonStr = fileResult as? String,
                   let data = try? JSONSerialization.jsonObject(with: Data(jsonStr.utf8)) as? [String] {
                    currentTags = data
                }
                if let jsonStr = allResult as? String,
                   let data = try? JSONSerialization.jsonObject(with: Data(jsonStr.utf8)) as? [String] {
                    allTags = data
                }

                // Merge: current tags first, then any vault tags not already present
                var displayTags = currentTags
                for t in allTags {
                    if !displayTags.contains(t) { displayTags.append(t) }
                }

                DispatchQueue.main.async {
                    let alert = UIAlertController(title: "File Tags", message: "Tap to toggle, or add new", preferredStyle: .actionSheet)

                    for tag in displayTags {
                        let isActive = currentTags.contains(tag)
                        let title = isActive ? "✓ \(tag)" : "  \(tag)"
                        alert.addAction(UIAlertAction(title: title, style: .default) { _ in
                            self.webView?.evaluateJavaScript("window.__myceliumToolbar?.tagSet(\(jsString(tag)))", completionHandler: nil)
                        })
                    }

                    alert.addAction(UIAlertAction(title: "+ Add New Tag", style: .default) { _ in
                        let input = UIAlertController(title: "New Tag", message: nil, preferredStyle: .alert)
                        input.addTextField { $0.placeholder = "tag name" }
                        input.addAction(UIAlertAction(title: "Add", style: .default) { _ in
                            if let tag = input.textFields?.first?.text?.trimmingCharacters(in: .whitespaces), !tag.isEmpty {
                                self.webView?.evaluateJavaScript("window.__myceliumToolbar?.tagSet(\(jsString(tag)))", completionHandler: nil)
                            }
                        })
                        input.addAction(UIAlertAction(title: "Cancel", style: .cancel))
                        self.presentAlert(input)
                    })

                    alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))

                    if let popover = alert.popoverPresentationController {
                        popover.sourceView = sender
                        popover.sourceRect = sender.bounds
                    }
                    self.presentAlert(alert)
                }
            }
        }
    }

    private func showDatePicker(for type: String, from sender: UIButton) {
        // Fetch existing date from JS so we can pre-select it
        let jsGet = "window.__myceliumToolbar?.getExisting?.(\(jsString(type))) ?? ''"
        webView?.evaluateJavaScript(jsGet) { [weak self] result, _ in
            guard let self = self else { return }
            let existingStr = result as? String ?? ""

            DispatchQueue.main.async {
                let vc = DatePickerViewController()
                vc.titleText = type == "deadline" ? "Set Deadline" : "Set Scheduled"

                // Pre-select existing date if available
                if !existingStr.isEmpty {
                    let formatter = DateFormatter()
                    formatter.dateFormat = "yyyy-MM-dd"
                    if let date = formatter.date(from: existingStr) {
                        vc.initialDate = date
                    }
                }

                vc.onDateSelected = { [weak self] date in
                    let formatter = DateFormatter()
                    formatter.dateFormat = "yyyy-MM-dd"
                    let dateStr = formatter.string(from: date)
                    let dayFormatter = DateFormatter()
                    dayFormatter.dateFormat = "EEE"
                    let dayStr = dayFormatter.string(from: date)
                    let timestamp = "<\(dateStr) \(dayStr)>"
                    let jsType = type == "deadline" ? "deadlineSet" : "scheduledSet"
                    self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.\(jsType)(\(jsString(timestamp)))", completionHandler: nil)
                }
                vc.onRemove = { [weak self] in
                    let jsType = type == "deadline" ? "deadlineSet" : "scheduledSet"
                    self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.\(jsType)(null)", completionHandler: nil)
                }
                vc.modalPresentationStyle = .pageSheet
                if let sheet = vc.sheetPresentationController {
                    sheet.detents = [.large()]
                }
                self.presentAlert(vc)
            }
        }
    }

    private func presentAlert(_ vc: UIViewController) {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let rootVC = scene.windows.first?.rootViewController else { return }
        var topVC = rootVC
        while let presented = topVC.presentedViewController { topVC = presented }
        topVC.present(vc, animated: true)
    }

    // MARK: - Accessory override

    /// Classes whose `inputAccessoryView` getter has already been replaced.
    private static var overriddenClasses = Set<ObjectIdentifier>()

    /// Make `webView`'s class answer `inputAccessoryView` with its toolbar.
    ///
    /// A web view with a toolbar gets the toolbar while the editor is focused
    /// and no accessory otherwise. Any other instance of the class — there are
    /// none today — keeps whatever it answered before.
    private static func overrideInputAccessoryView(of webView: WKWebView) {
        guard var cls: AnyClass = object_getClass(webView) else { return }
        // Key-value observation swaps in a hidden subclass of its own; the
        // override belongs on the class it was made from.
        while NSStringFromClass(cls).hasPrefix("NSKVONotifying_"), let parent = class_getSuperclass(cls) {
            cls = parent
        }
        guard !overriddenClasses.contains(ObjectIdentifier(cls)) else { return }

        let selector = #selector(getter: UIResponder.inputAccessoryView)
        guard let current = class_getInstanceMethod(cls, selector) else {
            NSLog("[Mycelium] Web view has no inputAccessoryView; toolbar not installed")
            return
        }

        typealias Getter = @convention(c) (AnyObject, Selector) -> UIView?
        let previous = unsafeBitCast(method_getImplementation(current), to: Getter.self)
        let getter: @convention(block) (AnyObject) -> UIView? = { receiver in
            if let toolbar = objc_getAssociatedObject(receiver, &toolbarAssociationKey) as? KeyboardToolbar {
                return toolbar.editorFocused ? toolbar : nil
            }
            return previous(receiver, selector)
        }
        let implementation = imp_implementationWithBlock(getter)

        // Usually the class inherits the getter from WKWebView, so this adds an
        // override to it alone. If it already declares one, replace that; the
        // previous implementation is kept above for web views without a toolbar.
        if !class_addMethod(cls, selector, implementation, method_getTypeEncoding(current)) {
            method_setImplementation(current, implementation)
        }
        overriddenClasses.insert(ObjectIdentifier(cls))
    }

    /// The view inside `view` that currently holds first responder, if any.
    private static func firstResponder(in view: UIView) -> UIView? {
        if view.isFirstResponder { return view }
        for subview in view.subviews {
            if let found = firstResponder(in: subview) { return found }
        }
        return nil
    }
}

/// Receives the page's editor focus reports.
///
/// The user content controller retains its handlers, so this holds the toolbar
/// weakly to keep the toolbar's lifetime tied to the web view alone.
private class EditorFocusHandler: NSObject, WKScriptMessageHandler {
    private weak var toolbar: KeyboardToolbar?

    init(toolbar: KeyboardToolbar) {
        self.toolbar = toolbar
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        toolbar?.setEditorFocused((message.body as? Bool) ?? false)
    }
}

// MARK: - Date Picker View Controller

/// A proper modal view controller with a UIDatePicker, Set/Remove/Cancel buttons.
/// Presented as a half-sheet so nothing overlaps.
class DatePickerViewController: UIViewController {
    var titleText: String = "Pick a Date"
    var initialDate: Date?
    var onDateSelected: ((Date) -> Void)?
    var onRemove: (() -> Void)?

    private let datePicker = UIDatePicker()

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        // Button stack — placed at top so it's always visible
        let setBtn = UIButton(type: .system)
        setBtn.setTitle("Set Date", for: .normal)
        setBtn.titleLabel?.font = .boldSystemFont(ofSize: 17)
        setBtn.backgroundColor = .systemGreen
        setBtn.setTitleColor(.white, for: .normal)
        setBtn.layer.cornerRadius = 10
        setBtn.addTarget(self, action: #selector(setTapped), for: .touchUpInside)

        let removeBtn = UIButton(type: .system)
        removeBtn.setTitle("Remove", for: .normal)
        removeBtn.titleLabel?.font = .systemFont(ofSize: 15)
        removeBtn.setTitleColor(.systemRed, for: .normal)
        removeBtn.addTarget(self, action: #selector(removeTapped), for: .touchUpInside)

        let cancelBtn = UIButton(type: .system)
        cancelBtn.setTitle("Cancel", for: .normal)
        cancelBtn.titleLabel?.font = .systemFont(ofSize: 15)
        cancelBtn.addTarget(self, action: #selector(cancelTapped), for: .touchUpInside)

        let btnStack = UIStackView(arrangedSubviews: [setBtn, removeBtn, cancelBtn])
        btnStack.axis = .horizontal
        btnStack.distribution = .fillEqually
        btnStack.spacing = 12
        btnStack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(btnStack)

        // Title label
        let titleLabel = UILabel()
        titleLabel.text = titleText
        titleLabel.font = .boldSystemFont(ofSize: 17)
        titleLabel.textAlignment = .center
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(titleLabel)

        // Date picker
        datePicker.datePickerMode = .date
        datePicker.preferredDatePickerStyle = .inline
        if let initial = initialDate {
            datePicker.date = initial
        }
        datePicker.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(datePicker)

        NSLayoutConstraint.activate([
            // Buttons at top
            btnStack.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 12),
            btnStack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            btnStack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),
            btnStack.heightAnchor.constraint(equalToConstant: 44),

            // Title below buttons
            titleLabel.topAnchor.constraint(equalTo: btnStack.bottomAnchor, constant: 12),
            titleLabel.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            titleLabel.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),

            // Date picker fills the rest
            datePicker.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 4),
            datePicker.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 8),
            datePicker.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -8),
        ])
    }

    @objc private func setTapped() {
        onDateSelected?(datePicker.date)
        dismiss(animated: true)
    }

    @objc private func removeTapped() {
        onRemove?()
        dismiss(animated: true)
    }

    @objc private func cancelTapped() {
        dismiss(animated: true)
    }
}
