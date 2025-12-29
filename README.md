## PlayMix

A Stream Deck plugin for Linux audio control and media playback, featuring per-application volume mixing with dynamic visual feedback.

Based on the [OpenAction MPRIS plugin](https://github.com/OpenActionPlugins/mpris) by [nekename](https://github.com/nekename).

### Features

#### Volume Dial with Smart Mixing
- **Encoder Press + Rotate**: Cycle through all audio-producing applications (master volume, individual apps)
- **Encoder Rotate**: Adjust volume of currently selected source
- **Dynamic Images**: Shows app icons or album art from MPRIS metadata
- **Per-Instance State**: Multiple dials can control different sources independently

#### Media Control Actions
- Play/Pause with album art display
- Stop
- Previous track
- Next track

### Audio Source Display

The volume dial automatically displays context-appropriate images:
- **Master Volume**: Shows volume icon
- **Individual Apps**: Shows application icon (Discord, Brave, etc.)
- **Media Players**: Displays album art from MPRIS when available
  - Note: Chromium browsers share one MPRIS instance per window, so multiple tabs playing media will show the browser icon to avoid confusion

### Requirements

- Linux with PulseAudio/PipeWire
- `pactl` for per-app volume control
- `wpctl` for master volume control
- MPRIS-compatible media players for metadata/album art

### Hardware & Platform

This plugin is designed for the **Soomfon CN003** Stream Deck alternative and runs on a modified version of the [opendeck-akp05](https://github.com/ambiso/opendeck-akp05) plugin by [ambiso](https://github.com/ambiso) which can be found [here](https://github.com/keinervonuns/opendeck-akp05).

**Key modification**: Dial presses toggle (press/release events) instead of only sending the down event when pressed multiple times.

### Included Application Icons

The plugin includes icons for common applications:
- **Brave** (`brave.png`)
- **Chrome** (`chrome.png`)
- **Discord** (`discord.png`)
- **Unknown/Fallback** (`unknown.png`)

Icons are from [Font Awesome](https://fontawesome.com/).

### Adding Custom Icons

To add icons for additional applications:

1. Create a **PNG image file** (program icons must be PNG format)
2. Name it after the application's process binary name (e.g., `spotify.png`, `firefox.png`)
3. Place it in `~/.config/opendeck/plugins/PlayMix.sdPlugin/icons/`
4. The plugin will automatically use it when that application is selected

The plugin searches for icons using the process binary name from PulseAudio sink input properties.

**Note**: While button action icons can be SVG, application icons displayed on the volume dial must be PNG format.

---

**Disclaimer**: This project was developed with assistance from AI coding tools.
