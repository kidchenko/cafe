# Homebrew cask. Lives in a tap (github.com/kidchenko/homebrew-tap) as
# Casks/cafe.rb, then: brew install --cask kidchenko/tap/cafe
#
# The release workflow rewrites `version` and `sha256` from the DMG it built.
cask "cafe" do
  version "0.0.1"
  sha256 "REPLACE_WITH_SHA256"

  url "https://github.com/kidchenko/cafe/releases/download/v#{version}/Cafe-#{version}.dmg"
  name "Cafe"
  desc "Keep your Mac awake from the menu bar"
  homepage "https://github.com/kidchenko/cafe"

  depends_on macos: ">= :big_sur"

  app "Cafe.app"

  # Ad-hoc signed, not notarized: without this, Gatekeeper refuses to open it.
  postflight do
    system_command "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "#{appdir}/Cafe.app"]
  end

  uninstall quit:      "com.kidchenko.cafe",
            launchctl: "com.kidchenko.cafe"

  zap trash: [
    "~/Library/Application Support/Cafe",
    "~/Library/LaunchAgents/com.kidchenko.cafe.plist",
  ]
end
