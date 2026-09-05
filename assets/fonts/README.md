# Terminal fonts

Kervesh reuses Hack Regular already embedded by egui 0.33.3. It adds the unmodified
Hack Bold 3.003 font (317,628 bytes), distributed by Source Foundry. The complete
MIT and Bitstream Vera notices are in [Hack-LICENSE.md](Hack-LICENSE.md).
Upstream: https://github.com/source-foundry/Hack

No Nerd Font is required. Users can choose an absolute local monospace TTF/OTF
path and ordered fallback paths on Windows or Linux. These files are not bundled
into exports; missing or invalid files fall back to Hack and produce diagnostics.
The default chain retains egui's existing symbol/emoji fallbacks after Hack.
