# ARCropolis

A modding framework for loading and managing community-made mods and plugins powered by [Skyline](https://github.com/skyline-dev/skyline) for Super Smash Bros. Ultimate. Made with love by [Raytwo](https://github.com/Raytwo) with help from [jam1garner](https://github.com/jam1garner), [blujay](https://github.com/blu-dev), [Coolsonickirby](https://github.com/Coolsonickirby), [Shadów](https://github.com/shadowninja108), and contributions by many more!

## Features

ARCropolis comes built-in with a few features such as:

- [Auto-updater](https://github.com/Raytwo/ARCropolis/wiki/Auto-updater)
- [File logger](https://github.com/Raytwo/ARCropolis/wiki/File-logging)
- [Mod manager](https://github.com/Raytwo/ARCropolis/wiki/Mod-manager)
  - If you'd rather manage your mods on a PC, consider using [Quasar](https://github.com/Mowjoh/Quasar) by [Mowjoh](https://github.com/Mowjoh), letting you download mods with a one-button press on GameBanana!
- [Workspace manager](https://github.com/Raytwo/ARCropolis/wiki/Workspaces-and-Workspace-Selector)
- Configuration editor
- Mod conflict handling
- [An API for plugin developers](https://github.com/Raytwo/arcropolis_api)

### Migration from Ultimate Mod Manager

To migrate mods from an Ultimate Mod Manager setup, rename `sd:/UltimateModManager` to `sd:/ultimate`, delete `rom:/data.arc`, and you're good to go!
If you need a guide explaining things step-by-step, consult the [wiki](https://github.com/Raytwo/ARCropolis/wiki/Overview-(Getting-started)).

### Work-in-progress emulator support

While only Ryujinx has some compatibility with ARCropolis for the time being, support for Yuzu is being worked on.  
Please be aware that we are not affiliated in any way with either of these emulators, so we are not responsible for any progress on that front.
If you are interested in using ARCropolis on a emulator, please read [the following](https://github.com/Raytwo/ARCropolis/issues/195) beforehand.

## Downloads

Head to the [releases](https://github.com/Raytwo/ARCropolis/releases/latest) page to get the latest build!  
Beta builds are sometimes posted there, but please be aware that you are expected to provide constructive feedback when using them.

### Installation and usage

A wiki page is available to help get you started with [setting up ARCropolis](https://github.com/Raytwo/ARCropolis/wiki/Overview-(Getting-started)).

### If you run into issues

1. Consider reading the [Troubleshooting](https://github.com/Raytwo/ARCropolis/wiki/Common-Issues-and-How-To-Fix-Them) section of the [wiki](https://github.com/Raytwo/ARCropolis/wiki) to find some pointers on what could have gone wrong.  
2. If you still can't manage to solve the problem, consider opening a [Discussion](https://github.com/Raytwo/ARCropolis/discussions/categories/issues) to get some assistance when possible.  
2.1. In the case where you are certain the issue comes from ARCropolis itself, consider opening an issue using one of our various [templates](https://github.com/Raytwo/ARCropolis/issues/new/choose) available. Use this as a last resort only, as we are flooded with bug reports that are user errors.

### Have an idea for improvements or new features?

Head over to the [Discussions](https://github.com/Raytwo/ARCropolis/discussions/categories/ideas) tab and suggest your idea(s)! If it sounds doable and useful, it might eventually make it into a future version of ARCropolis!

## Noteworthy mods and plugins for use with ARCropolis

- [HewDraw Remix (HDR)](https://github.com/HDR-Development/HDR-Releases) - A massive gameplay overhaul with cherry-picked skins by various community members, custom menus, new music, and much more!
- [Smash Minecraft Skins](https://github.com/jam1garner/smash-minecraft-skins) - A Skyline plugin for downloading Minecraft skins from its official servers directly from Smash!
- [One Slot Victory Theme](https://github.com/Coolsonickirby/One-Slot-Victory-Theme) - A Skyline plugin for configuring victory fanfares on a per-costume basis.
- [Arc Randomizer](https://github.com/Coolsonickirby/arc-randomizer) - A Skyline plugin for randomly picking one of multiple files when modding the game.

## Special Thanks

Here is a list of multiple people who have contributed to ARCropolis over time:

- Current maintainers: `Raytwo`, `blujay`
- Contributors: `Raytwo`, `Coolsonickirby`, `blujay`, `jam1garner`, `jozz`
- Special thanks: `Shadów`, `Genwald`
- Logo: `Styley`
