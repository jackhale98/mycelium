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
                        self.webView?.evaluateJavaScript("window.__myceliumToolbar?.todoSet('\(kw)')", completionHandler: nil)
                    })
                }
                for kw in doneKw {
                    alert.addAction(UIAlertAction(title: "✓ \(kw)", style: .default) { _ in
                        self.webView?.evaluateJavaScript("window.__myceliumToolbar?.todoSet('\(kw)')", completionHandler: nil)
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
                        self.webView?.evaluateJavaScript("window.__myceliumToolbar?.prioritySet('\(p)')", completionHandler: nil)
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

    private func showDatePicker(for type: String, from sender: UIButton) {
        let vc = DatePickerViewController()
        vc.titleText = type == "deadline" ? "Set Deadline" : "Set Scheduled"
        vc.onDateSelected = { [weak self] date in
            let formatter = DateFormatter()
            formatter.dateFormat = "yyyy-MM-dd"
            let dateStr = formatter.string(from: date)
            let dayFormatter = DateFormatter()
            dayFormatter.dateFormat = "EEE"
            let dayStr = dayFormatter.string(from: date)
            let timestamp = "<\(dateStr) \(dayStr)>"
            let jsType = type == "deadline" ? "deadlineSet" : "scheduledSet"
            self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.\(jsType)('\(timestamp)')", completionHandler: nil)
        }
        vc.onRemove = { [weak self] in
            let jsType = type == "deadline" ? "deadlineSet" : "scheduledSet"
            self?.webView?.evaluateJavaScript("window.__myceliumToolbar?.\(jsType)(null)", completionHandler: nil)
        }
        vc.modalPresentationStyle = .pageSheet
        if #available(iOS 15.0, *), let sheet = vc.sheetPresentationController {
            sheet.detents = [.medium()]
            if #available(iOS 16.0, *) {
                sheet.prefersGrabberIndicator = true
            }
        }
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let rootVC = scene.windows.first?.rootViewController else { return }
        var topVC = rootVC
        while let presented = topVC.presentedViewController { topVC = presented }
        topVC.present(vc, animated: true)
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

// MARK: - Date Picker View Controller

/// A proper modal view controller with a UIDatePicker, Set/Remove/Cancel buttons.
/// Presented as a half-sheet so nothing overlaps.
class DatePickerViewController: UIViewController {
    var titleText: String = "Pick a Date"
    var onDateSelected: ((Date) -> Void)?
    var onRemove: (() -> Void)?

    private let datePicker = UIDatePicker()

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

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
        datePicker.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(datePicker)

        // Button stack
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

        NSLayoutConstraint.activate([
            titleLabel.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            titleLabel.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            titleLabel.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),

            datePicker.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 8),
            datePicker.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 8),
            datePicker.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -8),

            btnStack.topAnchor.constraint(equalTo: datePicker.bottomAnchor, constant: 8),
            btnStack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            btnStack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),
            btnStack.heightAnchor.constraint(equalToConstant: 44),
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
