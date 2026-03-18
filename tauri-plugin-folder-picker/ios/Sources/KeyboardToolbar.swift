import UIKit
import WebKit

/// Native iOS keyboard toolbar that sits above the software keyboard.
/// Uses the inputAccessoryView mechanism — same approach as iA Writer, Bear, Blink.
///
/// Installation: call `KeyboardToolbar.install(on:)` with the WKWebView instance.
/// Each button tap calls evaluateJavaScript to trigger the web app's handlers.
class KeyboardToolbar: UIView {
    private weak var webView: WKWebView?

    /// Create and install the toolbar on a WKWebView.
    /// This replaces the default inputAccessoryView by swizzling WKContentView.
    static func install(on webView: WKWebView) {
        let toolbar = KeyboardToolbar(webView: webView)
        // Store toolbar reference on the webView to keep it alive
        objc_setAssociatedObject(webView, "myceliumToolbar", toolbar, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        // Install via WKContentView swizzle
        swizzleInputAccessoryView(toolbar: toolbar, in: webView)
    }

    private init(webView: WKWebView) {
        self.webView = webView
        super.init(frame: CGRect(x: 0, y: 0, width: UIScreen.main.bounds.width, height: 44))
        autoresizingMask = .flexibleWidth
        backgroundColor = .secondarySystemBackground
        setupUI()
    }

    required init?(coder: NSCoder) { fatalError() }

    // MARK: - UI Setup

    private func setupUI() {
        // Top border
        let border = UIView()
        border.backgroundColor = .separator
        border.frame = CGRect(x: 0, y: 0, width: bounds.width, height: 0.5)
        border.autoresizingMask = .flexibleWidth
        addSubview(border)

        // Scroll view for horizontal scrolling
        let scroll = UIScrollView()
        scroll.frame = CGRect(x: 0, y: 0.5, width: bounds.width, height: 43.5)
        scroll.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        scroll.showsHorizontalScrollIndicator = false
        scroll.showsVerticalScrollIndicator = false
        addSubview(scroll)

        // Button definitions: (label, action, color, bold, width)
        let buttons: [(String, String, UIColor?, Bool, CGFloat)] = [
            ("Link", "link", .systemGreen, true, 44),
            ("|", "", nil, false, 1),
            ("H", "heading", nil, true, 36),
            ("TODO", "todo", .systemRed, true, 48),
            ("[#]", "priority", .systemOrange, true, 38),
            ("DL", "deadline", .systemRed, false, 32),
            ("SC", "scheduled", .systemBlue, false, 32),
            ("|", "", nil, false, 1),
            ("B", "bold", nil, true, 32),
            ("I", "italic", nil, false, 32),
            ("U", "underline", nil, false, 32),
            ("S", "strike", nil, false, 32),
            ("~c~", "code", nil, false, 38),
            ("=v=", "verbatim", nil, false, 38),
            ("|", "", nil, false, 1),
            ("-", "list", nil, false, 28),
            ("☐", "checkbox", nil, false, 32),
            ("|T|", "table", nil, false, 38),
            ("SRC", "srcblock", nil, false, 42),
            ("❝", "quote", nil, false, 28),
            ("Date", "timestamp", nil, false, 42),
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

    // MARK: - Button Tap Handler

    @objc private func buttonTapped(_ sender: UIButton) {
        guard let action = sender.accessibilityIdentifier, !action.isEmpty else { return }

        switch action {
        case "todo":
            showTodoPicker(from: sender)
        case "heading":
            showHeadingPicker(from: sender)
        case "priority":
            showPriorityPicker(from: sender)
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
        let alert = UIAlertController(title: "Set TODO State", message: nil, preferredStyle: .actionSheet)
        // Get keywords from JS config, or use defaults
        let todoKw = ["TODO", "NEXT", "WAITING", "HOLD"]
        let doneKw = ["DONE", "CANCELLED"]

        alert.addAction(UIAlertAction(title: "None", style: .default) { [weak self] _ in
            self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.todoSet(null)", completionHandler: nil)
        })
        for kw in todoKw {
            alert.addAction(UIAlertAction(title: kw, style: .default) { [weak self] _ in
                self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.todoSet('\(kw)')", completionHandler: nil)
            })
        }
        for kw in doneKw {
            alert.addAction(UIAlertAction(title: "✓ \(kw)", style: .default) { [weak self] _ in
                self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.todoSet('\(kw)')", completionHandler: nil)
            })
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))

        if let popover = alert.popoverPresentationController {
            popover.sourceView = sender
            popover.sourceRect = sender.bounds
        }
        presentAlert(alert)
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
        let alert = UIAlertController(title: "Set Priority", message: nil, preferredStyle: .actionSheet)

        alert.addAction(UIAlertAction(title: "None", style: .default) { [weak self] _ in
            self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.prioritySet(null)", completionHandler: nil)
        })
        for p in ["A", "B", "C"] {
            alert.addAction(UIAlertAction(title: "[#\(p)]", style: .default) { [weak self] _ in
                self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.prioritySet('\(p)')", completionHandler: nil)
            })
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))

        if let popover = alert.popoverPresentationController {
            popover.sourceView = sender
            popover.sourceRect = sender.bounds
        }
        presentAlert(alert)
    }

    private func showDatePicker(for type: String, from sender: UIButton) {
        let alert = UIAlertController(title: type == "deadline" ? "Set Deadline" : "Set Scheduled", message: "\n\n\n\n\n\n\n\n\n", preferredStyle: .actionSheet)

        let datePicker = UIDatePicker()
        datePicker.datePickerMode = .date
        datePicker.preferredDatePickerStyle = .inline
        datePicker.translatesAutoresizingMaskIntoConstraints = false
        alert.view.addSubview(datePicker)

        NSLayoutConstraint.activate([
            datePicker.leadingAnchor.constraint(equalTo: alert.view.leadingAnchor, constant: 8),
            datePicker.trailingAnchor.constraint(equalTo: alert.view.trailingAnchor, constant: -8),
            datePicker.topAnchor.constraint(equalTo: alert.view.topAnchor, constant: 50),
        ])

        // Resize the alert to fit the date picker
        let height = NSLayoutConstraint(item: alert.view!, attribute: .height, relatedBy: .equal, toItem: nil, attribute: .notAnAttribute, multiplier: 1, constant: 480)
        alert.view.addConstraint(height)

        alert.addAction(UIAlertAction(title: "Set", style: .default) { [weak self] _ in
            let date = datePicker.date
            let formatter = DateFormatter()
            formatter.dateFormat = "yyyy-MM-dd"
            let dateStr = formatter.string(from: date)

            let dayFormatter = DateFormatter()
            dayFormatter.dateFormat = "EEE"
            let dayStr = dayFormatter.string(from: date)

            // Format as org timestamp: <2024-01-15 Mon>
            let timestamp = "<\(dateStr) \(dayStr)>"
            let jsType = type == "deadline" ? "deadlineSet" : "scheduledSet"
            self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.\(jsType)('\(timestamp)')", completionHandler: nil)
        })
        alert.addAction(UIAlertAction(title: "Remove", style: .destructive) { [weak self] _ in
            let jsType = type == "deadline" ? "deadlineSet" : "scheduledSet"
            self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.\(jsType)(null)", completionHandler: nil)
        })
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))

        if let popover = alert.popoverPresentationController {
            popover.sourceView = sender
            popover.sourceRect = sender.bounds
        }
        presentAlert(alert)
    }

    private func presentAlert(_ alert: UIAlertController) {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let rootVC = scene.windows.first?.rootViewController else { return }
        var topVC = rootVC
        while let presented = topVC.presentedViewController { topVC = presented }
        topVC.present(alert, animated: true)
    }

    // MARK: - WKContentView Swizzle

    /// Install the toolbar by swizzling the inputAccessoryView on WKContentView.
    /// This is the standard technique used by a-shell, Blink, and other WKWebView apps.
    private static func swizzleInputAccessoryView(toolbar: KeyboardToolbar, in webView: WKWebView) {
        guard let contentView = findContentView(in: webView) else {
            NSLog("[Mycelium] Could not find WKContentView for toolbar installation")
            return
        }

        // Store toolbar on the content view
        objc_setAssociatedObject(contentView, "myceliumToolbarView", toolbar, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)

        let contentViewClass: AnyClass = type(of: contentView)

        // Create a dynamic subclass to override inputAccessoryView
        let subclassName = "MyceliumContentView"
        if let existingClass = NSClassFromString(subclassName) {
            // Already swizzled, just update the toolbar reference
            object_setClass(contentView, existingClass)
            return
        }

        guard let subclass = objc_allocateClassPair(contentViewClass, subclassName, 0) else {
            NSLog("[Mycelium] Failed to create dynamic subclass for toolbar")
            return
        }

        // Override inputAccessoryView getter
        let getterSel = NSSelectorFromString("inputAccessoryView")
        let getterBlock: @convention(block) (AnyObject) -> UIView? = { obj in
            return objc_getAssociatedObject(obj, "myceliumToolbarView") as? UIView
        }
        let getterImp = imp_implementationWithBlock(getterBlock as Any)
        let typeEncoding = "@@:" // returns object, takes self + _cmd
        class_addMethod(subclass, getterSel, getterImp, typeEncoding)

        objc_registerClassPair(subclass)
        object_setClass(contentView, subclass)

        NSLog("[Mycelium] Keyboard toolbar installed successfully")
    }

    /// Walk the view hierarchy to find the WKContentView.
    private static func findContentView(in view: UIView) -> UIView? {
        let className = NSStringFromClass(type(of: view))
        if className == "WKContentView" {
            return view
        }
        for subview in view.subviews {
            if let found = findContentView(in: subview) {
                return found
            }
        }
        return nil
    }
}
