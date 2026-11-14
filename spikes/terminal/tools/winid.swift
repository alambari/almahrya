import CoreGraphics
let owner = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "terminal-spike"
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
for w in list {
  if let o = w[kCGWindowOwnerName as String] as? String, o == owner,
     let n = w[kCGWindowNumber as String] as? Int {
    print(n); break
  }
}
