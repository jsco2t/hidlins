import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  private var attachmentExport: HidlinsAttachmentExport?

  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)
    let attachmentExport = HidlinsAttachmentExport()
    attachmentExport.register(with: flutterViewController.engine.binaryMessenger)
    self.attachmentExport = attachmentExport

    super.awakeFromNib()
  }
}
