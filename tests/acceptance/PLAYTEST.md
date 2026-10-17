# Play-test checklist

Play the first level with `rusty-claw --install PATH` (title, main menu, new
game). Note only what fails: the line, what happened instead, and where.
Where a line says "as in the original", compare with the original game.

1. Strike an officer standing, crouching and in the air: each blow lands when the sword or fist reaches him, he flinches and slides back, and he dies as quickly as in the original.
2. Let an officer strike you: one hit sound and pain line, a short push back, then a moment in which he cannot hurt you again.
3. Stand at range from a soldier: he aims and fires a musket ball (10 health), and clubs you up close; the pistol and the sword kill him.
4. Stand near a rat: it throws a bomb that explodes on you; a crouching swipe kills it, and a rat walking into a wall turns back.
5. Fire the pistol, magic and dynamite (hold the fire key to throw dynamite farther): each uses one shot, the status bar counts it down, and with none left nothing leaves.
6. Lift a powder keg (Z), carry it, throw it at an enemy: it explodes and kills him; carrying feels as slow as in the original.
7. Break crates with the sword: their contents bounce out and can be picked up; stacked crates above drop down.
8. Pick up coins, treasure, food, ammunition and a power-up: the score, health, ammunition and power-up timer change, and treasure flies to the score corner.
9. Step on a crumbling peg and time how long it holds you before it falls away: the same as in the original.
10. Ride the first elevator over the spikes and climb the ladder at its end; cross the toggling pegs before the map piece: each peg comes out on its beat and holds you while out.
11. Fall onto spikes after a checkpoint: Claw dies there, the circle closes, he comes back at the last flag with full health and one life fewer, and the pegs and elevators behind him are back in place.
12. Lose every life: the game over screen fades in, then the main menu.
13. Touch a save point, end the game from the in-game menu, then load that save point: Claw starts there with the same score, lives and ammunition.
14. Take the map piece: the music stops, the map and the treasure tally count up what you collected against what the level holds, then the main menu shows level 2 open.
15. At 3840x2160 (`--res 3840x2160 --fullscreen`): the status bar is sharp and sits at the screen's corners; nothing in the play area blurs or shimmers while scrolling.
16. On an ultrawide display (or `--res 5120x1440`): the status bar sits at the outer edges (centred in the 4:3 area with the status bar video option), enemies in the extra width stay still until Claw comes near.
17. Listen to a fight: hits, shots and speech are clear over the music, no sound cuts out in a busy moment, and sounds to the left or right of Claw come from that side.
18. Walk past torches and ambient sounds: they fade in and out with distance; pausing silences them and resuming brings them back.
19. With the SoundFont installed: the level music plays, a power-up switches to its tune and back to the level tune where it left off, and the menus play their own background.
20. In the options, change the sound and music volumes, the speech and ambient switches, and the window size, aspect and scaling: each applies at once and is still set after a restart.
21. Rebind a key and a pad button in the controls page: the new binding works in play, a key taken from another action swaps with it, and the defaults entry restores the original keys.
22. On a Steam Deck: the d-pad and left stick move Claw, the stick does not drift him when centred, A jumps, B strikes, X fires, Start opens the menu, and the menus follow the d-pad.
23. Suspend the window (switch away): play pauses and resumes where it was, with no stuck keys or pad buttons.
24. Lift an officer's body (one killed on spikes, within the second it lies there, or one lifted the moment a blow kills him), then throw it: it is carried and flies like a live officer, hurts nobody, bounces, and disappears a second after it stops, with no second loot; a body Claw drops lies still, never gets up, and disappears a second later.
25. Get hit in mid-air (a soldier's shot or your own dynamite), then press jump before landing: Claw jumps again, as in the original; with the mid-air hit jump fix on in the options, he cannot.
26. Hit an officer with magic: compare the burst with the original (today no lingering flame or frost shows on the enemy).
27. Charge a keg throw on a crumbling peg until it gives way: the keg drops from Claw's hands as he falls; after any throw the keg never stays in his hands.
28. Get hit with speech switched off in the options: the hit sound still plays and Claw's pain line is silent.
29. Take catnip and let it run out: every sparkle around Claw disappears at once when it ends.
30. Start the busiest fight you can (several enemies, shots and explosions at once): no sound you would miss stops early (past 64 sounds at once, the oldest short one is cut to make room).
31. Watch a rat walk into a wall: it turns back and walks away at its usual pace, as in the original.
32. Open Options in the in-game menu and in the main menu, then Display in each: GAMEPLAY and VIDEO are large, centred entries on rows of their own between AUDIO (or the last display entry) and BACK, the coins flank them when highlighted, and the Gameplay and Video pages they open show their rows in large text too.
33. Not on level 1, so not checked here: enemies stepping into goo (they take it like spikes), springboards (Claw's jump height, and enemies on them do not launch), loot carried through a warp, and enemies on moving elevators (they stay put).

## Milestone 3, slice 0 (LEVEL1)

Failures only, as above: note the line, what happened instead, and where.
`PATH` is your install; `make compare` also needs `CLAW_REFERENCE`, the
folder with the reference traces of the first level's routes.

Known differences from the original on LEVEL1, not to report. They are
explained in `tests/acceptance/tolerances/README.md`, and those a compared
route meets are listed with their reasons in
`tests/acceptance/tolerances/references.toml`:

- The random draw schedule: every third game step the original draws more
  random numbers than the game, so from about 0.2 s into a route enemies
  can choose differently (for example an officer's strike that hits Claw
  in one game and misses in the other). It is still to be matched.
- Feel at 60 frames per second: a fall from standing still runs up to
  about one tick ahead of the original mid-fall, and a running jump with
  the jump key held only part of the way peaks 11 px lower. A route played
  blind at 60 frames per second drifts from there and need not finish the
  level in both games.
- Left or Right held from before play starts: in the original the held
  direction counts again once Claw's state changes (for example after a
  jump), in the game only once it is released and pressed again.

No longer a difference: Claw climbs up at 220 px/s, as in the original.
Report a climb that feels faster or slower.

1. Watch the bot's run of the first level: `make run-game ARGS="--install PATH --level 1 --replay tests/acceptance/routes/level1.csv"`. Claw goes from the start to the map piece without a death you would not expect, and the level ends.
2. `CLAW_INSTALL=PATH CLAW_REFERENCE=... make compare REF=level1/bot-original-v2` and `make compare REF=level1/bot-v2` (the original's runs of the current routes, at `c18` and `c60`) each end with "no divergence beyond the known ones"; each divergence printed is one `tests/acceptance/tolerances/references.toml` lists for that route, with its reason.
3. In LEVEL1, press Pause: the game freezes with the small PAUSE sign and no menu, and the sounds and music stop. Press Pause again: play goes on and the music carries on where it stopped. Escape does nothing while paused.
4. Tap Pause twice quickly, several times: each resume advances the game by exactly one tick. Holding Pause steps at the keyboard repeat rate.
5. Pause, press and hold Jump, then resume: Claw jumps on the first tick. Hold Jump before pausing and keep holding it through the resume: no new jump.
6. Hold Jump through the in-game menu or help and close it: no jump until Jump is released and pressed again.
7. Throw dynamite, open help (F1) for about 2 s, close it: the stick goes off at once and its blast reaches Claw about 8 ticks later. With `FUSE CLOCK FROZEN` on the Gameplay page, the stick waits for its full fuse instead. The in-game menu and the pause always freeze the fuse.
8. Pause, press F1, close help: play runs with no PAUSE sign, sound effects and ambient play, the music stays silent, and Escape is blocked. Press Pause once: the music carries on from where it stopped, sounds playing at that moment are not cut, and Escape works again.
9. On the Gameplay page, choosing `PAUSE INPUT` switches between `ORIGINAL` and `RELEASE ALL`, and `FUSE CLOCK` between `ORIGINAL` and `FROZEN`. Both are saved to the settings file at once and apply from the next level started. A recording keeps the values it was made with.
10. Watching a script or running a benchmark (no menus), a gamepad's Start pauses and resumes like the Pause key. In normal play it opens the in-game menu.
11. Mid-air stand window: hit Claw in the air, then jump on the first tick after landing. With `MID AIR HIT JUMP FIX OFF` (the default), the one-tick window behaves as in the original, and pause stepping can land the jump in it.
12. With 3 lives, choose end life in the in-game menu and confirm: Claw dies where he stands, falls off the screen and comes back at the last checkpoint with 2 lives. Try it while climbing and while carrying a keg (he lets go of it).
13. With 1 life, choose end life: the question warns that this ends the game; confirm, and the game over screen follows. Answering no goes back to play with nothing changed.
14. Turn the front layer off in the options: the level's front tiles (the foreground in front of Claw) disappear while objects still draw. Quit and start the game again: the front layer is still off. Turn it back on.
15. Start LEVEL1 and LEVEL2 (`--level 2`): every powder keg stands on the floor when the level starts, none floats above it or sinks into it.

## Milestone 3, slice 1: LEVEL2 early

Failures only, as above: note the line, what happened instead, and where.
Start the second level with `make run-game ARGS="--install PATH --level 2"`.
The boss at the end is not built yet: stop before his room.

1. Every powder keg stands on the floor when the level starts; none floats above it or sinks into it.
2. Walk up to the forts beside the stone towers: each punk rat paces a short beat behind its little cannon, squeaks now and then, and every four seconds or so strikes at the back of its beat; its cannon fires a ball as the strike ends and the rat recoils, as in the original.
3. A punk rat never hurts you by touch, and any blow (sword, kick, pistol, dynamite, magic) kills it at once; its body flies off, it drops nothing, and its cannon stays on the ledge and never fires again. A pistol shot that kills it flies on.
4. With the invisibility power-up running, punk rats pace without striking.
5. The tower cannons fire a ball every three seconds, whether or not you are near, with their shot sound; climbing the ladder between a fort and its tower, the balls of the rats (from the left) and of the tower (from the right) cross at alternating heights, as in the original.
6. A cannonball flies straight, costs you 10 health (also while you still flash from another hit), passes through enemies and kegs without harming them, and disappears at the first wall.
7. Throw an officer or a soldier into a goo pool (or knock one in): it drops its loot, plays the goo sound and is gone at once, never seen standing in the goo; a rat dropped in sinks without a sound.
8. The goo surface pictures over the pools are drawn in front of Claw and the enemies, so whatever falls in disappears behind the goo.
9. The arrow signs that point the way cycle through their pictures, about ten a second.
