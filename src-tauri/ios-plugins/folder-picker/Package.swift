// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "FolderPickerPlugin",
    platforms: [.iOS(.v15)],
    products: [
        .library(name: "FolderPickerPlugin", targets: ["FolderPickerPlugin"])
    ],
    dependencies: [
        .package(name: "Tauri", path: "../../gen/apple/.tauri/tauri-api")
    ],
    targets: [
        .target(
            name: "FolderPickerPlugin",
            dependencies: [
                .product(name: "Tauri", package: "Tauri")
            ],
            path: "Sources"
        )
    ]
)
