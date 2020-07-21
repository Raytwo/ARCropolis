# ARCropolis

A Skyline plugin for replacing arbitrary files in Smash Ultimate with files of arbitrary size. Made in equal parts by Raytwo and jam1garner, with lots of help from Shadow as well. In-game textures (nutexb) are not currently support.


**Warning:** Not everything will work. Report issues to the issues tab.

### Installation

Copy the zip from the latest release onto the SD so the plugin is in the following folder:

```
    sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/
```

### Usage

Place your files in the following folder:

```
    sd:/atmosphere/contents/01006A800016E000/romfs/arc/
```

For example, if you want to replace the file

```
    ui/message/msg_melee.msbt
```

you place it in

```
    sd:/atmosphere/contents/01006A800016E000/romfs/arc/ui/message/msg_melee.msbt
```

**Note:** In some Arc tooling, regional files will show up with a `+us_en` (or another region code) at the end of the filename. Do not include this.

### Ultimate Mod Manager Backwards Compatibility

The plugin also supports backwards compatibility with UMM paths to allow for mods to continue to work.

Currently, to prevent issues with nutexb-related crashes, it is opt-in. In the future, this may be removed.

While UMM stores files in `sd:/UltimateModManager/mods`, ARCropolis stores its modpack-style mods in `sd:/ultimate/mods` while maintaining the same folder structure as UMM. If none of your files are `.nutexb` textures, you can simply rename your `UltimateModManager` folder to `ultimate` and all mods will work. If you no longer need UMM, it is recommended you delete your data.arc file from romfs.
