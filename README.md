# ARCropolis

A [Skyline](https://github.com/skyline-dev/skyline) plugin for replacing any file in Super Smash Bros. Ultimate by placing mods on your SD card.
Made by Raytwo with help from jam1garner, blujay, Coolsonickirby, and Shadow. Currently maintained by jam1garner.

### Installation and usage
A Wiki is available to help get you started with [setting up ARCropolis](https://github.com/Raytwo/ARCropolis/wiki/Overview-(Getting-started)).

### Features
ARCropolis comes built-in with a few features such as:
* [Auto-updater](https://github.com/Raytwo/ARCropolis/wiki/Auto-updater)
* [File logger](https://github.com/Raytwo/ARCropolis/wiki/File-logging)
* [Workspace manager](https://github.com/Raytwo/ARCropolis/wiki/Workspace-selector)
* [Mod manager](https://github.com/Raytwo/ARCropolis/wiki/Mod-manager)

### Backward compatible with Ultimate Mod Manager
Simply rename your ``sd:/umm/`` directory to ``sd:/ultimate/``, delete your ``data.arc`` and you're good to go!  
If you need a guide explaining things step-by-step, consult the [Wiki](https://github.com/Raytwo/ARCropolis/wiki/Overview-(Getting-started)).

### Downloads
Head to the [release](https://github.com/Raytwo/ARCropolis/releases/latest) page to get the latest build!  
Beta builds are sometimes posted there, too.

### Special thanks
Here is a list of the multiple people who have contributed to ARCropolis over time

Current Maintainer: ``jam1garner``

Former Maintainer: ``Raytwo``

Developers: ``Raytwo``, ``CoolSonicKirby``, ``blujay``, ``jam1garner``

Additional Contributors: ``Shadow``, ``Genwald``

Logo: ``Styley``  

# To-Do Before Unsharing Release
1. Fix voice files. Issue as described by JoeTE:
> Voice/sound stuff was a bit odd though. Doing dittos with any character seems to mute all voice clips & sound effects for all but the 1st instance of the character if the original voice clips & sound effects were shared files.

> So for example, if I had both Female Octoling & Female Inkling loaded in the same match, the Octoling will have the unique voice & sound effects, while the Inkling would have no voice & no sound effects

> But having both a Female Octoling & Male Octoling would result in both having voices, but the Male Octoling lacking sound effects.

2. Fix the memleaks that are happening [here](https://github.com/blu-dev/arcrop-unshared-development/tree/master/src/res_list.rs#L84). I'm pretty sure that the game does not handle each of these as unique entries, and as such frees them as a memory block. In order to solve that, we will need to reallocate the entire list. Not *horrible*, but not very nice. Probably generate a range of `LoadInfo` during the unsharing process that we can reference later on so we aren't doing that every time we need it.

3. Once the above issues are fixed, would be nice to distribute to a few select modders as testers before pushing an official release

4. As a general rule, let's clean up the code. With what we know, we might be able to clean up the code and make it more approachable by people new to the scene.