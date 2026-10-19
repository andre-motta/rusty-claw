# Play-test checklist

Play the first level with `rusty-claw --install PATH` (title, main menu, new
game). Note only what fails: the line, what happened instead, and where.
Where a line says "as in the original", compare with the original game.

For hard levels, the play-test aids (`--invincible`, `--infinite-lives`, or
the Gameplay page) let you reach and repeat any part of a level, and
`--powerup NAME` or F6 (with an aid on) give any powerup; a run with an aid
on says nothing about how hard the level is.

1. Strike an officer standing, crouching and in the air: each blow lands when the sword or fist reaches him, he flinches and slides back, and he dies as quickly as in the original.
2. Let an officer strike you: one hit sound and pain line, a short push back, then a moment in which he cannot hurt you again.
3. Stand at range from a soldier: he aims and fires a musket ball (10 health), and clubs you up close; the pistol and the sword kill him.
4. Stand near a rat: it throws a bomb that explodes on you; a crouching swipe kills it, and a rat walking into a wall stays against it.
5. Fire the pistol, magic and dynamite (hold the fire key to throw dynamite farther): each uses one shot, the status bar counts it down, and with none left nothing leaves.
6. Lift a powder keg (Z), carry it, throw it at an enemy: it explodes and kills him; carrying feels as slow as in the original.
7. Break crates with the sword: their contents bounce out and can be picked up; stacked crates above drop down.
8. Pick up coins, treasure, food, ammunition and a power-up: the score, health, ammunition and power-up timer change, and treasure flies to the score corner.
9. Step on a crumbling peg and time how long it holds you before it falls away: the same as in the original.
10. Ride the first elevator over the spikes and climb the ladder at its end; cross the toggling pegs before the map piece: each peg comes out on its beat and holds you while out.
11. Fall onto spikes after a checkpoint: Claw dies there, the circle closes, he comes back at the last flag with full health and one life fewer, and the pegs and elevators behind him are back in place.
12. Lose every life: the game over screen fades in, then the main menu.
13. Touch a save point, end the game from the in-game menu, then load that save point: Claw starts there with the same score, lives and ammunition.
14. Take the map piece: the music stops, the map and the treasure tally count up what you collected against what the level holds, then the second level starts with your score, lives, health and ammunition (the main menu shows level 2 open too).
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
30. Start the first level with a play-test aid on (`--invincible`), then press F6 once for each powerup in turn; each press names the powerup in the middle of the screen, switches to the powerup tune and starts its status bar timer at 30 seconds. INVISIBLE: Claw is drawn see-through, the scenery showing through him; give it away from enemies, then walk up to one: enemies that had not noticed him do not come for him or shoot, as in the original (one already fighting him may keep striking). INVULNERABLE: Claw is a dark silhouette whose shade flickers about ten times a second, and shots and blows leave his health alone (play with `--infinite-lives` alone to see that, since `--invincible` hides it). CATNIP: Claw walks and climbs faster and jumps higher, sparkles trail him, and his blows hurt twice as much. FIRE SWORD, ICE SWORD, LIGHTNING SWORD: every swipe sends that sword's shot with its sound, and the shot hurts an officer. NO POWERUP: the look ends at once, the sparkles go and the level tune comes back. Losing a life ends any look at once; `--powerup fire` gives the fire sword again at the start and after the respawn, and F6 does nothing without an aid on.
31. Start the busiest fight you can (several enemies, shots and explosions at once): no sound you would miss stops early (past 64 sounds at once, the oldest short one is cut to make room).
32. Watch a rat walk into a wall: it stays against the wall for good, its walk playing on the spot, until it throws a bomb at Claw, as in the original.
33. Open Options in the in-game menu and in the main menu, then Display in each: GAMEPLAY and VIDEO are large, centred entries on rows of their own between AUDIO (or the last display entry) and BACK, the coins flank them when highlighted, and the Gameplay and Video pages they open show their rows in large text too.
34. On level 2, knock an officer or a soldier into a goo pool: it is gone at once, as on spikes (line 4 of the LEVEL2 section). Springboards are checked on the fourth level (its early section below); not checked yet: loot carried through a warp, and enemies on moving elevators (they stay put).

## Wide screens

Failures only, as above. Play on a display wider than 4:3 (or with
`--res 3840x2160` or `--res 5120x1440`) in the default fill aspect.

1. Shoot or strike an enemy near the side of the screen so that its body flies into the extra width: it plays its whole death fall there and leaves at the bottom of the screen, instead of vanishing at the edge of the 4:3 area.
2. Walk up to crates, crate stacks, powder kegs and pickups that start in the extra width: each is there, whole and on the floor, before Claw comes near, and does not pop in, jump down or change when he gets close (a stack shows all its crates; a pickup shows its sparkle).

## Milestone 3, slice 0 (LEVEL1)

Failures only, as above: note the line, what happened instead, and where.
`PATH` is your install; `make compare` also needs `CLAW_REFERENCE`, the
folder with the reference traces of the first level's routes.

Known differences from the original on LEVEL1, not to report. They are
explained in `tests/acceptance/tolerances/README.md`, and those a compared
route meets are listed with their reasons in
`tests/acceptance/tolerances/references.toml`:

- Feel at 60 frames per second: a fall from standing still runs up to
  about one tick ahead of the original mid-fall, and a running jump with
  the jump key held only part of the way peaks 11 px lower. A route played
  blind at 60 frames per second drifts from there and need not finish the
  level in both games.
- Left or Right held from before play starts: in the original the held
  direction counts again once Claw's state changes (for example after a
  jump), in the game only once it is released and pressed again.

No longer a difference: Claw climbs up at 220 px/s, as in the original.
Report a climb that feels faster or slower. The random draws now follow
the original's schedule: at one 18 ms step per frame the game equals the
original's run of the earlier bot route on every step, enemies' choices
included. Standing, Claw walks when Left or Right is pressed (or when one
is still held after an attack ends), and after a lost life he is back at
the checkpoint as soon as the circles are over, free to move at once.

1. Watch the bot's run of the first level: `make run-game ARGS="--install PATH --level 1 --replay tests/acceptance/routes/level1.csv"`. Claw goes from the start to the map piece without a death you would not expect, and the level ends.
2. `CLAW_INSTALL=PATH CLAW_REFERENCE=... make compare REF=level1/bot-original-v3` and `CLAW_INSTALL=PATH CLAW_REFERENCE=... make compare REF=level1/bot-v3` (the original's runs of the earlier bot routes, at `c18` and `c60`) each end with "no divergence beyond the known ones"; each divergence printed is one `tests/acceptance/tolerances/references.toml` lists for that route, with its reason.
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
16. Lift a powder keg with Z, then press Z again and hold it: Claw holds the wind-up pose until Z comes up, then throws. A tap throws the keg about 141 px, a 1.5 s hold about 275 px and a 3 s hold about 440 px; held for 3 s, he throws by himself.

## Milestone 3, LEVEL2

Failures only, as above: note the line, what happened instead, and where.
Start the second level with `make run-game ARGS="--install PATH --level 2"`,
or play on from the first level.

Known differences from the original on LEVEL2, not to report (listed with
their reasons in `tests/acceptance/tolerances/references.toml`):

- After the end-of-level screen the original's enemies make the choices
  they make every third tick in a phase the wall clock sets; the game
  starts every level in the phase a fresh start gives.
- Claw's falling death drifts by a pixel from the original's mid-fall.

1. Every powder keg stands on the floor when the level starts; none floats above it or sinks into it.
2. The punk rats by the stone towers pace behind their little cannons and strike every four seconds or so; each strike fires the cannon, a ball that costs you 10 health. Any blow kills a rat at once and its cannon falls silent.
3. The tower cannons fire a ball every three seconds, whether or not you are near; on the ladder between a fort and its tower the rats' and the tower's balls cross at alternating heights, as in the original.
4. Throw or knock an officer or a soldier into a goo pool: it drops its loot, plays the goo sound and is gone at once; a rat dropped in sinks without a sound. Goo is drawn in front of whatever falls in.
5. Walk into Raux's room: the boss music starts, Claw stands frozen while the camera visits Raux, both speak, the banner drops and leaves, and both move again together, as in the original. The boss meter shows full and falls with every blow that lands; Raux's defeat throws the gem, and taking it ends the level on the amulet screen with the gem set in its socket, then the third level starts.
6. Jump on the first moment in Raux's room (the jump-in): no introduction, Claw is never frozen, and Raux stands idle in his start-up pose while you stand beside him; swinging at him from close range wakes him and he fights normally, as in the original.
7. On LEVEL2, throw dynamite and open help (F1) for about 2 s: the stick goes off at once on closing, as on LEVEL1. Get hit in mid-air by a soldier's ball or a cannonball and jump on the first tick after landing: Claw jumps again, as in the original.
8. Play the first level and go on into the second in one sitting (new game): the score, lives, health and ammunition carry over, and the second level plays to Raux's defeat and the third level's start without a stall.

## Milestone 3, LEVEL3 early

Failures only, as above: note the line, what happened instead, and where.
Start the third level with `make run-game ARGS="--install PATH --level 3"`,
or play on from the second level. This section covers the level's new
hazards, Claw's rope swing and the two thieves.

1. Floor spikes: each row rises and sinks on its own beat, forever, with a sound as each spike starts up and down when it is on screen; rows of mixed spikes rise as a wave. A risen spike costs 10 health and knocks Claw back, then cannot hurt him again for a moment; a spike moving up or down, or down, never hurts. Only the bottom of a risen spike hurts: standing just beside one is safe, as in the original.
2. Path elevators: the two groups past the first save point follow their loops (an eight-sided loop of four platforms, two five-sided loops of five) at a steady speed and never bump into each other. Jump on one and ride it all the way round: Claw stays on it on every leg, the slanted ones included, and walks and jumps off it as off any elevator.
3. Rope swing, grab: jump or fall into the lower end of a swinging rope: Claw catches it and hangs just below the end. Standing or climbing under a rope's end, he does not catch it.
4. Rope swing, swing: hanging on, Claw swings with the rope and turns to face the way it swings at each end of the swing; he passes through walls and floors with it, as in the original.
5. Rope swing, let go: press jump: Claw leaps from where he hangs with no push from the swing, and Left or Right steers the leap. He cannot catch the same rope again for half a second (another rope at once). Cross the first pit on its two ropes, one to the next.
6. Rope swing, get hit: let a shot or an enemy hit Claw while he swings: he takes the damage (10 for a thief's knife or arrow) and shows the hurt pose for a single frame, then the rope catches him again at once, so he stays on it, as in the original.

7. Thieves: while Claw is level with a cutthroat on screen with nothing between them, it stands still and throws a knife about every 2 s, slashes with its sword when Claw comes close, and never walks up to him; a robber thief shoots arrows the same way. Each shouts once when it first sees Claw.
8. A knife or an arrow that reaches Claw costs 10 health and is gone; one that reaches a wall stops there.
9. Knock a thief out and lift it with Z: Claw carries it and throws it as he does an officer, and a thrown thief hurts the enemies it lands on. A beaten thief drops the loot it carries and stops attacking once it starts to fall.
10. Watch a route or a recording in the window (`--replay FILE.csv`): the level music and the game's sounds play from the level's start as in play, the boss and powerup tunes come in where play would start them, and sound carries on after the recording ends and the keyboard takes over. Where the recording paused or opened a menu, the sounds stop and the music holds, then both carry on as in play. The Pause key and switching to another window pause the sound as in play; `--no-audio` (or `audio = off`) keeps the replay silent.
11. Rope swing, smooth: on a 60 Hz display, swing on a rope: Claw, the rope's end and the level around him move smoothly on every frame, with Claw always on the rope's end. Start with `--smooth-rope off` (or `smooth_rope = off`): the swing goes back to the original's stepped look, the view standing still on some frames (about one in three on a rope of the usual speed).

## Milestone 3, LEVEL4 early

Failures only, as above: note the line, what happened instead, and where.
Start the fourth level with `make run-game ARGS="--install PATH --level 4"`,
or play on from the third level. This section covers the springy ferns,
the stepping stones over the goo, the level's floor spikes and goo, and
Katherine, the level's boss.

1. Ferns, standing: land on a fern and stand still: about a third of a second later it bends with its spring sound and throws Claw straight up, far higher than his own jump, to the same height on every throw (the first fern, just right of the start, throws him back onto itself again and again). Some ferns throw lower or higher than others, as in the original.
2. Ferns, walking off: land on a fern and walk off its edge at once: Claw drops off without being thrown, and the fern still bends on its own.
3. Ferns, keys: hold Jump while landing on a fern and through the throw, or walk back and forth on it: the throw comes at the same moment and to the same height as when standing still; a jump of your own before the throw is an ordinary jump.
4. Ferns, sound: every few seconds the ferns rustle, several at once where they stand close together (at the level start, four rustle together), as in the original.
5. Stepping stones, the clock rows: each stone sinks into the goo and rises again on its own beat, a row sinking as a wave; a stone stops holding Claw the moment it starts to sink, and holds him again once it is halfway back up. Arrive late and Claw drops through into the goo and dies. The stones make no sound.
6. Stepping stones, the ones that wait: a stone that waits for Claw sinks a moment after he lands on it (jump on at once to keep going), stays under for about a second and comes back up, ready again.
7. Floor spikes: each spike rises and sinks on its beat, as on the third level, and hurts only while up; goo pools kill Claw and swallow enemies dropped in, drawn in front of whatever sinks.
8. Katherine, introduction: jump into the door to Katherine's clearing: the boss music starts, Claw stands frozen while the camera visits her, both speak, the banner drops and leaves, and when both move again she attacks at once with her whip from about 160 px. The boss meter shows full and falls with every blow that lands; she blocks most sword blows and somersaults over Claw now and then, as in the original.
9. Katherine, jump-in: jump with Right held on the first moment in her clearing (the jump-in): no introduction and no freeze. She wakes while Claw is in the air and fights normally, and Claw comes down on her and loses 5 health. Walking back into the spot where Claw arrived later starts the whole introduction mid-fight; her next hit unfreezes Claw.
10. Katherine, lift: stand close in front of her, more than about 20 px and less than about 85 px between you, and press Lift: Claw picks her up and throws her, and she bounces, rests, loses a little health and gets up to fight again. Pressed right up against her, Lift does nothing.
11. Katherine, the gem: defeat her: the gem rises to a point above the clearing's floor, Claw is held until it lands, and a plain jump from the floor under it collects it and ends the level.

## Milestone 3, LEVEL5 early

Failures only, as above: note the line, what happened instead, and where.
Start the fifth level with `make run-game ARGS="--install PATH --level 5"`,
or play on from the fourth level. This section covers the two town guards
(the lighter first guard and the bulkier second guard), the jump switches
that launch them, being hit in mid-air, the seagulls, the breaking planks
and the statues.

1. Guards, idle: walk up to a guard and stand still near it without swinging: it plays its idle poses for about two to four seconds before it comes for you, as in the original; step out of its reach and back, and it starts its idle poses over.
2. Guards, strikes: standing, a guard's cut lands about a third of a second into its swing, with its strike sound, costs 10 health and pushes Claw back a little; it keeps striking while you stay close, sometimes stepping in or backing off first.
3. Guards, crouching: crouch in front of a guard: it cuts low instead, a tick later in its swing; crouching and facing away just beyond its reach, Claw is not hit until it steps in, as in the original.
4. The hop back: swing the sword at an idle second guard while facing it: it shouts and hops about 100 px back from where it stood (not from Claw), lands, and comes back to fight. Keep swinging: it hops again, at most three times in a row. A first guard does not hop on this level until it has gone back to pacing once.
5. The parry: keep striking a second guard: about one blow in four it raises its blade and the blow does nothing (a block sound, sometimes a shout); if it was walking at Claw it slides on during the parry. Otherwise it flinches, and the second blow kills it, flying off and dropping its coin. A first guard never parries here.
6. Lifting a second guard: Z lifts it; while Claw lifts it the guard is drawn in his hands (it does not vanish for a moment), then over his head. Throw it: it bounces twice, rests, gets up and comes back for you.
7. Jump switches: stand below the raised ledge right of the start, out of the guard's reach, and watch it pace: when it walks onto the switch near the ledge's left end it leaps off through the air and lands on the floor below, then comes for you; it never paces back to its old patrol. A guard Claw has just struck walks over a switch without leaping.
8. Hit in mid-air: jump into a guard's strike: Claw hangs where he was struck for a moment, stands there in mid-air, then falls; a jump pressed in the moment he stands launches a full jump, as in the original (and as for a soldier's ball, line 25 at the top).
9. Seagulls: each seagull flies back and forth along its stretch of sky at a steady speed, through walls, turning at the ends. When Claw stands below and ahead of one (a little more than 100 and less than 240 px down, and further away across than down), it screeches and dives in a straight line aimed just above his head, never steering. Its beak knocks him back for 10 health. It then flies up to the end of its stretch and patrols on. It never hurts him outside a dive.
10. Seagulls, in the air: jump into a dive. The beak knocks Claw out of the air, he hangs for a moment and stands in the air, and a Jump pressed right then gives a full jump.
11. Seagulls, killing: any blow, shot or thrown object kills a seagull at once with a squawk. It flips, pops up and falls away through the floor, dropping any treasure it carries.
12. Breaking planks: stand or walk on a plank. After half a second (three quarters on the slow rows, a quarter on the fast ones) it crumbles with a creak, its floor goes about halfway through and Claw drops. The planks beside it then go one after another along the row. Standing in the small gap between the middles of two planks starts nothing. Jumping over a plank starts nothing, but landing on one does. Guards on a row never break it, but they fall when it goes.
13. Planks after a death: lose a life after breaking a row. When Claw comes back, every plank of the row is whole again and breaks again.
14. Statues: one sword swing, shot or thrown object breaks a statue. It crumbles over about a second into a pile of rubble that stays, and throws out a fan of treasure that bounces and can be collected. From the right spot, one swing breaks both statues of the pair near the start. Claw walks behind the front statues (drawn over him) and in front of the others, and the front statues' treasure flies in front of him too.

## Milestone 3, LEVEL6 early

Failures only, as above: note the line, what happened instead, and where.
Start the sixth level with `make run-game ARGS="--install PATH --level 6"`,
or play on from the fifth level. This section covers the goo vents in the
sewers, the ground blowers in the streets and on the rooftops, and the
town guards' differences on this level, and Wolvington's room at the end.

1. Goo vents, timing: watch a vent in the sewers (the first two are down below and right of the start): it swells, sprays a flat splash sideways up to about 150 px wide and sinks back, then starts again at once, about every 3.7 s; the splash is up for about 2.4 s of each eruption. It makes no sound of its own (the bubbling is the level's ambient sound).
2. Goo vents, damage: stand in the splash: each hit costs only 1 health, Claw flinches and is pushed about 27 px away from the vent; still in the splash, he is hit again just under a second later, until the pushes take him out of its reach. Beside the vent out of the splash's reach, or with his feet a little above the floor, he is never hurt.
3. Ground blowers, standing: stand on a blower's nozzle: about once a second the jet hisses and throws Claw straight up, about 450 px (two blowers throw about 400), with no push sideways; standing on the nozzle again after landing, he is thrown again about every 1.5 s (1.4 s on the lower ones).
4. Ground blowers, walking in: walk back and forth across a nozzle: Claw is thrown only when the jet starts while he is over it, so not on every crossing; thrown out of a walk, he rises as high as from standing.
5. Ground blowers, jumping in: jump on a nozzle, or fall onto it: when the jet starts while his feet are near the nozzle (within about 20 px of the floor), the throw replaces his own jump and takes him 450 px above where he is then; higher up, the steam column does nothing.
6. Ground blowers, holding jump: hold Jump on a nozzle through several throws: every throw comes at the same moment and to the same height as standing still.
7. Ground blowers, carrying: lift a keg (two lie close to blowers) and stand on a nozzle with it: the throw makes Claw let go of it.
8. Ground blowers, the hiss: each blower near Claw hisses once per jet, including blowers just off the screen, as in the original.
9. Town guards on this level: the first guard parries about one sword blow in four and hops back from Claw's sword from the first swing; the second guard parries about one blow in two; a few guards stand guard and never pace.

10. Wolvington, the normal fight: walk through the door and the introduction runs: the camera pans to him, his line, Claw's line, the name banner, then the fight. He walks or runs in, sprints into an uppercut, or stands and casts magic shots across the room. Crouch under the high shots and jump the low ones. Up close he uppercuts a standing Claw and thrusts low at a crouching one. Close sword swings make him hop back. Each blow takes 5 off his 100 on the health bar.
11. Wolvington, the intro skip: jump with Right on the first tick in the room (Jump held about 12 ticks). There is no introduction and no health bar, and he stays in his idle pose while you stand on the floor; touching him hurts you and he stays asleep. Holding Jump one tick longer, or the same jump with Catnip, wakes him in mid-air. A blow, a lift, a blocked shot, or walking back into the box by the door wakes him; the box plays the whole introduction first.
12. Wolvington, lifting him: stand close, facing him, and lift him, asleep or awake. Thrown, he bounces, loses 4, and starts a magic volley as soon as he gets up.
13. Wolvington, the gem: when he falls, Claw freezes and the gem rises above his place. Once it lands Claw can move. A long jump (Jump held about 11 ticks) collects it and ends the level; a short press falls short.

## Milestone 3, LEVEL7 early

Failures only, as above: note the line, what happened instead, and where.
Start the seventh level with `make run-game ARGS="--install PATH --level 7"`,
or play on from the sixth level. This section covers the red-tail pirates,
the bear sailors and the bear hug, and the crabs, their nests and their bombs.

1. Red-tail pirates, engaging: walk up to a pirate: it comes for you as soon as you are in its reach, with no idle routine first; from 100 to 200 px away it sometimes swings at the air and then waits about two seconds before striking again; up close it never stands and waits, it strikes or steps in or back.
2. Red-tail pirates, strikes: standing, its cut lands about an eighth of a second into the swing (10 health, a short push back) and it cuts again about every third of a second while you stay close; crouch in front of it and it cuts low instead, a little faster.
3. Red-tail pirates, the hop back: swing the sword or fire the pistol at a pirate in front of you, close in: it hops back about 100 px from where it stood as your attack starts, before the blow can land; keep attacking and it hops again, at most three times, then it takes the hits. Attacking it from behind, or from further than 100 px, never makes it hop.
4. Red-tail pirates, hits: it never parries on this level; a blow makes it flinch for about a third of a second, the second sword blow or pistol shot kills it, and it drops its coin.
5. Bear sailors, approach: a bear runs at you from afar, walks the last stretch and stops about 100 px from a standing Claw; there it stands in its home pose for a third to two thirds of a second, or swings its overhead blow, which lands about 0.3 s into the swing (10 health) and reaches you even standing just in front of it.
6. Bear sailors, being grabbed: walk to within about 50 px of a bear: it grabs you whatever you are doing, you freeze where you stand for a third of a second, then it pulls you in front of it, facing it, and squeezes; your health drops one point about every tenth of a second with no hit sound or flinch, and you show the three squeezed poses in step with the bear.
7. Bear sailors, mashing out: while squeezed, press keys: a new press on ten ticks of the squeeze frees you (alternating two keys, about a fifth of a second; one key tapped, about a third of a second); presses during the grab itself do not count, and nine presses are not enough.
8. Bear sailors, timing out: do nothing: after three seconds and 28 health it lets go, you stand, drop to the floor, and it grabs you again after its next pause if you stay close; a squeeze that empties your health kills you in its arms.

9. Crab nests: walk onto the sand hole of a nest (the first is near x 15468): on the hatch you hear a boink and three crabs arc out, one to the left, one slightly left, and one in a random direction. A nest never hatches again, even after Claw dies.
10. Crabs, snipping: stand next to a crab: it crouches, then charges or snips; a snip costs 5 health. Hitting a crab while it is out of its shell (charging, snipping or throwing) kills it at once.
11. Crabs, knocking shells: swing at a crab that is hiding or crouched: it slides about 37 px away per standing swing, with one small boink, and does not die; only the magic claw kills a crab fully withdrawn into its shell.
12. Crab bombs: a dark bomber crab lobs a bomb at Claw at most every 4 s; it bounces and explodes 1 s after the throw, or at once if it touches him (5 health). The blast also kills crabs caught out of their shells, the thrower included.
13. Crabs, lifting: walk up to a crab hiding in its shell and press Lift (or Up with Attack): Claw picks it up without a sound from it and carries it over his head, withdrawn in its shell; throw it at a pirate: it hurts the pirate (and breaks a crate it flies into) but passes through other crabs, lands, and carries on as a crab. A crab out of its shell (crouching, charging, snipping) cannot be lifted.

## Milestone 3, LEVEL8

Failures only, as above: note the line, what happened instead, and where.
Start the eighth level with `make run-game ARGS="--install PATH --level 8"`,
or play on from the seventh level. This section covers the level's
pirates and Gabriel's room at the end: Gabriel on his deck, his bombs, the
pirates he calls in, his cannon with its lever and the button on the mast.

1. Gabriel, the introduction: walk through the door and stand still: the camera pans up to Gabriel on his raised deck, his line, Claw's line, the name banner, then the fight, about 13.7 s after you arrive. He never comes down to fight you.
2. Gabriel, the bombs: from his idle pose (about 2.4 or 2.6 s) he sometimes says a line, waits half a second and lobs three bombs to his left on three of four lanes, the nearest lane landing near his ship and the farthest at the dock's west end; each bomb explodes when it comes down to the dock (5 health in the air and in the blast), and a bomb that hits you in the air explodes there.
3. Gabriel, the pirates: sometimes he calls a red-tail pirate: it appears on his deck behind him, runs left along the deck, leaps down onto the dock and fights you there; with two of his pirates alive he only waits another idle pose. When Gabriel is beaten, his pirates fall at once.
4. Gabriel, the cannon: sometimes he faces right and pulls his lever: the lever flips and the cannon under the bow fires a ball along the dock (10 health). Climb the three pegs at the dock's west end and strike the red-and-green button on the mast with the sword: it stays lit, the cannon turns its barrel up, and his next lever pull fires straight up through the deck into him; then the cannon turns back down and the button blinks again.
5. Gabriel, hits: sword, pistol, magic and blasts only make him shrug and say a line, and cost him nothing; a cannonball from below costs him 20 of his 100 on the health bar, and he answers with his bombs.
6. Gabriel, the fifth cannonball: he falls to his knees, turns about, leaps off the deck onto the dock, collapses, and the gem rises out of him while Claw stands frozen; once the gem settles above the dock Claw can move, and a jump collects it and ends the level. His empty health bar stays on screen.
7. Gabriel, the intro skip: jump with Right on the first tick in the room. There is no introduction and no health bar, Claw lands on the dock, and Gabriel stays asleep in his idle pose for as long as you leave him: no bombs, no pirates, no cannon. The button can still be struck, and the cannon then waits with its barrel up.
8. Gabriel, what wakes him after a skip: a blow makes him shrug and go idle, and while the blows keep coming he only shrugs, never attacks; walking back into the box by the door plays the whole introduction and then the normal fight; losing a life puts Claw back at the level's start and Gabriel goes idle and attacks alone in the empty room.
9. Gabriel, the lift-and-drown kill (with Catnip): with Catnip, the skip jump lands Claw on the deck beside him (his touch costs 10). Lift him (he keeps his idle picture while carried) and throw him onto the dock (4 off his health); for 1.5 s after he lands he cannot be lifted, then lift him again on the dock and hold the throw key for a full throw west: he flies into the water, is gone, and the gem rises from the water to above the dock.
10. Gabriel, after a throw he survives: on the dock he gets up, pauses a few times, and then stays in his strike pose for good: he never bombs, calls a pirate or pulls the lever again, and his strike hurts you only if you stand right on him.
11. Gabriel, the cannon after a lost life: strike the button, then lose a life before he fires: after the respawn the cannon shows its barrel down and the button blinks, but every later lever pull still fires straight up into him, and the cannon never turns down again.
12. Red-tail pirates on this level: stand next to one and keep swinging the sword: about one swing in four is parried (the pirate raises its sword with a clang and takes no damage, then fights on); the other swings hurt it as on the seventh level, and two hits kill it. In Gabriel's fight his bomb blasts also hurt the pirates he calls in: they flinch.

## Milestone 3, LEVEL9 early

Failures only, as above: note the line, what happened instead, and where.
Start the ninth level with `make run-game ARGS="--install PATH --level 9"`,
or play on from the eighth level. This section covers the hook-handed
pirates and PegLeg, the rolling gunner; the level's traps, belts, saws and
skull cannons have their own lines below.

1. Hook-handed pirate, the hold: walk up to a hook-handed pirate: from 100 to 300 px away he sometimes raises his hook in his parry pose and holds it for about two seconds without moving; walk into him while he holds it and his whole body hurts (10 health), again a little under a second later if you stay against him.
2. Hook-handed pirate, the swing: from the same range he sometimes swings his hook at the air instead; it reaches only about 67 px, so it hurts you only if you walk into it, and after a swing or a hold he waits about two seconds before the next.
3. Hook-handed pirate, the stab: he runs at you from afar, walks the last stretch and stops about 100 px from a standing Claw; closer than 100 px he stabs with his dagger about once a second (the stab lands about a tenth of a second in, 10 health).
4. Hook-handed pirate, parries: stand and swing the sword at him: every standing swing is parried with a clang, he takes nothing and fights on about a fifth of a second later. Crouch and stab, or jump and stab, or punch: those land (a crouching stab costs him 2 of his 10), and he swings his hook back at once instead of flinching; five crouching stabs kill him.
5. Hook-handed pirate, the pistol: shoot him while he walks or stands: he flinches and loses 8, and the second shot kills him. Shoot him while he holds the hook: he parries, the hold ends, and the bullet flies on past him.
6. PegLeg, the shot: stand 150 to 200 px from PegLeg (the first is on the deck right of the start, past a saw blade): he aims his pistol, the bullet leaves about a third of a second later from in front of him, high at a standing Claw and low at a crouching one, flies at about 500 px/s and costs 10; he cannot fire again for a second after a bullet.
7. PegLeg, the rolls: come closer than 150 px and he rolls about 150 px away from you in about 0.6 s, then shoots; from further off he walks or rolls towards you, stopping about 35 px short of you, or on the end of his patrol, and shoots. While he rolls nothing hurts him.
8. PegLeg, hits: shoot him with the pistol: it does him no harm and he rolls at you. Stand and swing: about half the swings make him flinch (5 of his 7); a crouching stab always lands (2), and for two seconds after it every blow lands.
9. PegLeg, lifting: lift him in the middle of a roll towards you (stand right of him, facing him, and press Lift as he arrives): he is carried like any pirate; throw him: he bounces, pauses half a second, loses no health, then taunts and fires six shots in a row about half a second apart, alternating high and low.

10. Belts: standing on a belt, Claw drifts with it from about a quarter second after landing; walking with a belt is faster, against it slower; a jump keeps the belt's push on the way up but not on the way down. Carried to the end of a belt over a drop, he teeters at the edge instead of falling. A warp touched on the fast belt leaves him standing still at its target. The pieces animate; only the ambient hum is heard.
11. Saws: sunk saws rise, spin and sink on a steady rhythm and hurt only once fully up; one hit costs 10, and at most two hits fit in one up period; neighbouring saws are offset so there is always a gap; no rise or sink sound, only the hum.
12. Skull cannons: they fire every 3 seconds whether Claw is near or not; the left-facing ones fire from behind themselves, so the ball passes through the cannon; the ball does not spin; it hits for 10 even when Claw ducks, and jumping over it works.
13. Dart traps: invisible until Claw stands in a trap's area; then a puff, a blow sound, and an arrow about half a second later from the hole; it hits for 5 and knocks Claw back, and repeats while he stays (about every 1.6 s).

## Milestone 3, LEVEL10

Failures only, as above: note the line, what happened instead, and where.
Start the tenth level with `make run-game ARGS="--install PATH --level 10"`,
or play on from the ninth level. This section covers the level: the
hook-handed pirates and PegLegs with their tenth-level ways, the dart traps, the pegs, the big trap elevators, the
seagulls and the vertical warp, and Marrow's room past the door: Marrow,
his parrot and the two slabs that can bridge the spike pit between the
ledges.

To start in front of the door with the tools, `--start 32340,7740` puts
Claw on the floor of the niche left of it, 92 px from the door (the bot
and `make replay ARGS="--install PATH --level 10 --start 32340,7740
--script ROUTE.csv"` take it; the game starts the level at its start).

1. Hook-handed pirate: he never raises his hook to hold it and never parries. Stand about 100 px from him and he stabs with his dagger about once a second, missing a standing Claw facing him; step in and swing the sword standing: the swing lands (5 of his 10) and he strikes back at once (10 health, Claw is knocked back), and a second swing that lands kills him.
2. PegLeg, the pairs: stand 150 to 200 px from a PegLeg: each time he aims he fires two shots about half a second apart, the first high and the second low, both leaving about 55 px in front of him; every pair is high then low, and each shot costs 10.
3. PegLeg, the rolls: come closer than 150 px and he rolls about 150 px away from you in about 0.6 s, then fires a pair. Shoot him with the pistol: it does him no harm and he rolls at you the full 150 px, through where you stand (his touch costs 10), and fires his pair from there, often past you.
4. Dart traps: the level's traps shoot left, right and down (none up): a puff, a blow sound, and an arrow half a second later that costs 5 and knocks Claw back, again while he stays in the trap's area.
5. Crumbling pegs: a crumbling peg gives way a moment after Claw lands on it and stays gone until he loses a life; toggle pegs appear and vanish on their own rhythm, four of them with their own longer on and off times.
6. Big trap elevators: the wide trap elevators carry Claw across their whole surface (about 180 px), the narrow elevators only their own; the one vertical trap elevator rides between its two stops at a steady pace once Claw steps on it.
7. Seagulls: the nine seagulls patrol and dive at Claw as on the ninth level.
8. The vertical warp: the disc at the top of the shaft near the level's start (x 4252, y 8348) takes Claw to the middle of the level, just above a checkpoint flag (x 20250, y 7097).
9. The door into Marrow's room (x 32432, y 7769): walking in takes Claw to the left ledge of the room; the introduction plays and Claw stands again after it.

10. Marrow, the room: past the door the two slabs between the ledges slide apart under the ledges in about 0.6 s, uncovering the spike pit; they do so whether or not the introduction runs. The parrot climbs from its perch above Marrow to a line near the top of the room and patrols it, end to end in about 2.1 s.
11. Marrow, the introduction: stand still after the door: the camera pans right to Marrow on the far ledge, his line, Claw's line, the name banner, then the fight, about 13.5 s after you arrive. Marrow paces on his ledge out of view and does not come for you; the parrot dives at no one before the fight is on.
12. The parrot's swoops: in the fight, at the end of its line on the side away from you, the parrot often turns down to your height and swoops straight across at you (10 health), flies on to the other end of its line and climbs back. A sword hit on a swoop makes it flinch for about 0.3 s and fly on; it never loses health; the third hit sends it home to its perch.
13. The bridge: as the parrot settles on its perch the slabs slide together into a bridge below the ledges (about 0.6 s, the right one first). Walk across and jump up onto Marrow's ledge.
14. Marrow's pistol: whenever you are on his height and on screen he fires every 1 to 4 s: the bullet leaves about 65 px in front of him, crosses the room at about 600 px/s and costs 20. Touching him costs 10. He blocks every pistol shot, stick of dynamite, magic shot and thrown object, which are used up.
15. Marrow, 25 damage: sword hits cost him 5 of his 100 on the health bar, and some blows are blocked (a clang). After five hits he blocks every blow, then whistles: no hit lands on him, the parrot flies to him, lifts him and carries him in an arc to the ledge away from you while the slabs slide apart under the bridge. Stand on the right slab then: it carries Claw into the ledge wall and slides out from under him, into the spikes. Every 25 damage the loop repeats: three parrot hits close the bridge again.
16. Marrow, the intro skip: jump with Right on the first tick in the room: there is no introduction and no health bar, Claw lands on the left ledge, and Marrow only stands and paces on his ledge: he never shoots and never comes at you. The parrot patrols high over the open pit and never dives. Walking back into the box by the door plays the whole introduction late.
17. The dynamite crossing after the skip: throw a stick of dynamite in the air toward the pit, jump off the left ledge's edge into its blast, and press Jump with Right on the tick after the hit's one stand (pausing and stepping helps): Claw jumps out of the blast onto Marrow's ledge. Twenty sword hits of 5 kill the skipped Marrow; between hits he flinches and strikes back, then holds his last pose. Pausing through the crossing changes nothing.
18. The gem: Marrow's body flies off and the gem rises out of him to a point over the middle of the pit in about 2.7 s while Claw stands frozen; then Claw can move, and a jump from the ledge's edge into the gem ends the level.
19. A lost life in the room after the skip: Claw comes back with Marrow at full health, the slabs snapped shut and sliding open again. From then on, every 25 damage Marrow blocks every blow and whistles, stays out of reach of every hit for 5 s waiting for a parrot that never comes, then takes blows again.
20. The bridge trick, a quicker kill: with Marrow 25 below where his last whistle left him (five hits, or a sixth blow he blocks), run left and jump off his ledge as he whistles, so that you are in the air over his half of the bridge when the parrot starts to carry him; land on the left slab as it slides apart, ride it and jump from it onto the left ledge before it slides under the rock. The parrot drops Marrow on the left ledge beside Claw and does not come back to its perch: the slabs stay open, his later whistles go unanswered (each time he waits 5 s out of reach of every hit, then fights on), and you kill him there; the gem rises over the pit and a jump from the left ledge's edge into it ends the level. Three hits on the parrot there send it home and bring the normal loop back. Standing still on the bridge instead does not work: standing on his half, Claw rides the right slab into the wall.

## Milestone 3, LEVEL11 early

Failures only, as above: note the line, what happened instead, and where.
Start the eleventh level with `make run-game ARGS="--install PATH --level 11"`,
or play on from the tenth level. This section covers what the level
reuses, its lasers, Mercat, the Fish and the Siren.

To try the end of the level with the tools, `--start 3424,1467` puts
Claw on the ledge under the start room, left of its pillar (`make replay
ARGS="--install PATH --level 11 --start 3424,1467 --script ROUTE.csv"`
takes it; the game starts the level at its start).

1. Lasers: in the six low corridors the two crystal clusters wait about 1.5 s, then arc lightning for about 1.35 s with crackling zaps, heard even off screen. The beam hurts from the moment the crystals light up until a moment after the arc is gone (1 health, Claw knocked back); standing in it costs 1 and again just under a second later, and that knock carries him out. Neither ducking nor jumping gets under or over a firing laser: cross while it waits. The laser at x 12516 draws over Claw, the others behind him.
2. Break planks: walking onto a plank row, a piece gives way only when Claw's middle is over it (a 30 px window per piece, nothing in the 34 px between pieces); it shakes, crumbles and drops him about a quarter to three quarters of a second later, and the next piece starts as the previous one crumbles, with the crumbling bridge sound.
3. Toggle pegs: the wall pegs slide in and out on a steady three-second rhythm in five pictures (about a quarter second each way), solid only while out; a few stay out for good and three keep their own offset.
4. Water: in the wading pools Claw walks on the pool floor with the water drawn over his legs (he looks waist-deep), with a splashing step sound along the floors and in the puddles; nothing slows him.
5. Deep pools: falling into a deep pool (the wide one at x 6912 to 8959 below the first checkpoint) kills Claw at once, as spikes do, with the spike death sound.
6. The ledge under the start room: dropping through the opening in the start room's floor, Claw falls past the ledge and cannot reach it, even holding Left; he lands on the ground below.
7. The way to the map piece: the ladder at x 3100 leads up to the left part of that ledge; walk right to below the pocket (x 3424) and jump: Claw takes the map piece and the level ends.

8. Mercat's bolts: whenever a Mercat can see Claw on his floor, he waits one to two seconds, raises his trident and throws a bolt of lightning straight at Claw at waist height with a crackle, at any distance he can see him from, without walking at him. The bolt costs 20 and bursts into a short explosion where it hits; a jump clears it, a crouch does not. A wall stops it without a burst, and Claw's respawn clears every bolt in flight.
9. Fighting a Mercat close in: swinging at him from in front within about 150 px makes him hop about 96 px back, calling out one of his hop lines, almost always the first time and less often after each hop (never after the third); a crouching sword stab is always parried with a clang and does nothing; standing stabs, punches and kicks land (two kill him), and about one pistol shot in four is parried. When he cannot see Claw (a wall between them, or Claw invisible), he walks or runs at him instead and, within about 150 px, stabs (10, reaching about 110 px).
10. Standing right in front of a Mercat (about 60 px) facing him: his bolt bursts on Claw at once and its explosion kills the Mercat himself; Claw loses only the bolt's 20. A Mercat that threw instead of stabbing at a close Claw stops hopping back from swings.
11. The Fish's leap: standing in a wading pool above a fish's swimming line, a fish swimming towards Claw leaps out with a splash, bites at the top of an arc about 150 px high and 250 px long, and drops back in with a splash on the far side.
12. The Fish's dash: with Claw level with a fish's line, it dashes along the pool floor to where he stood and swims on. Every touch of a fish costs 10, whatever it is doing; any single hit (sword, punch, pistol, magic, an explosion) kills it.
13. Pit walls: from the bank right of the pool at x 13854 to 14498, a fish leaping at Claw bounces off the pit's wall and falls back into its pool; none lands on the bank.

14. The Siren: come within about 500 px of a Siren on her floor (and within about 40 px up or down) and she faces you and sings (farther than about 200 px she sometimes walks at you instead), through walls too: a call, and four rings of sound a third of a second apart that drift towards you slowly and fade after about 300 px. She sings again about 5 s after her last call (never sooner than 4 s). Touching her costs 10; two sword hits kill her, and she can be picked up and thrown.
15. Her scream: a ring that reaches Claw, standing or crouching, costs no health (only his hit sound) and freezes his controls for about two seconds, each ring starting the two seconds again: a walk stops, a crouch stands up, a jump on its way up loses the rest of its height, and he shows a dazed pose. He can still be hurt and killed while frozen.
16. Keys held through the freeze act the moment it ends: holding Jump jumps at once, holding Right walks on at once, holding Down crouches again.
17. The pause key, the help screen (F1) and the in-game menu still open while Claw is frozen, and the freeze waits while they are up; losing a life while frozen does not end it: it runs on after the respawn.

18. The floor clip at the first fish pool (x 4067 to 4450): walk left to the pool's left wall, crouch there and stay down while the fish knock Claw back and forth. A touch from his right slides him, still crouched, under the ledge left of the wall; when the hit ends he stands up inside the floor and falls straight through it to the hall below the pool, landing near x 4000. A stick of dynamite blowing up between Claw and the wall pushes him away from it instead, and there is no fall.

## Milestone 3, LEVEL12 early

Failures only, as above: note the line, what happened instead, and where.
Start the twelfth level with `make run-game ARGS="--install PATH --level 12"`,
or play on from the eleventh level. This section covers what the level
reuses (the Mercats, Sirens and Fish of the eleventh level, the pegs, the
rocks and the floor spikes), its stalactites, its earthquakes and the
tentacles, and the boss fight with Aquatis, his crack, his stalactites
and the dynamite that waits in his room.

To try the water's edge with the tools, `--start 3110,3567` puts Claw on
the ledge at the water's edge, right of the first stalactites (`make
replay ARGS="--install PATH --level 12 --start 3110,3567 --script
ROUTE.csv"` takes it; the game starts the level at its start).

1. Stalactites: walk under a hanging stalactite (walk left along the ledge at the water's edge, x 3110, to about x 3070): a moment later it snaps loose with a crack, falls (about 200 px in under half a second), lands with a crash, crumbles in about a third of a second and is gone. The two beside it fall with it. Standing under one as it lands costs 5 and a flinch; walking or jumping into a hanging one costs 5 too, and it stays where it hangs.
2. Fallen stalactites stay gone until Claw loses a life; then every one hangs again where it was.
3. A pistol shot fired at a hanging stalactite flies through it and on; nothing Claw has can break or bring one down.
4. Earthquakes: dropping down the first shaft (from its lip at x 2925) Claw passes an invisible earthquake trigger: the ground rumbles (one of five rumbles, sometimes none) and the screen shakes for 5 s. The trigger does nothing more for 7 s; a Claw still in its area then gets the rumble and the shake again. Pausing during a shake does not shorten it.
5. Tall shafts: falling down the tall shaft at x 25540, the earthquake at its foot starts only when Claw is near the bottom, not as he enters the shaft at the top; its hanging stalactites hit him on the way down (5 each, a flinch each).
6. The water: falling into the deep water (left of the ledge at the water's edge, x 2600 to 2880, and all along the level's bottom) kills Claw at once, with the splash sound, as his feet reach the surface.
7. The ledge's end: standing at the very end of the ledge at the water's edge (x 2884), Claw teeters, and the dynamite key does nothing there; a step back from the edge, it throws as usual.
8. The Mercat over the first shaft: as Claw drops into the shaft from its lip, the Mercat on the far lip throws a bolt that hits him on the way down (20 and a flinch), as on the eleventh level.
9. The toggle peg beside Aquatis's rock (x 37781) stays out for good: it is solid to stand on at any time.

10. The tentacles: come within about 200 px left of a tentacle (on the ledge, or in the passage under the water): after a second or so it either whips sideways at Claw with the swipe sound (5, reaching about 136 px to its left at its own height; under the water it misses) or grabs him. Grabbed, Claw freezes where he is; about a third of a second later he is lifted out beside the tentacle (50 px right of it, 70 px up), facing right, crushed in its grip, and squeezed: one health point about every 0.2 s, 14 in all, until it lets go after 3 s and he drops.
11. Mashing frees Claw: pressing a key on ten ticks while squeezed (any game key) lets him go on the tenth press.
12. A second tentacle can grab Claw while the first still squeezes him: it lifts him beside itself and both squeeze him; a strike from a third tentacle hits him out of both holds.
13. Any hit (sword, pistol, magic, a blast) cuts a tentacle down: it sinks into the water with its death sound and grows back after 20 s (at once when Aquatis starts a frenzy). Nothing hurts it while it grabs or squeezes.
14. Aquatis's frenzy: every tentacle thrashes, swiping left and right in turn, and cannot be hurt; facing right, its swipe reaches about 136 px to its right, so near the ledge's right end it can knock Claw into the water.

15. Aquatis, the introduction: walk through the door at the end of the passage: Claw freezes in the room, the camera pans right to Aquatis, his line, back to Claw, Claw's line, then the name banner drops with a thud and slides off; about 13.8 s after arriving the fight is on. Aquatis then does nothing at all until Claw walks right along the ledge (to about x 37250); from then on he idles, says a line now and then, and punches high or low when Claw is within about 200 px left of him at his height.
16. The intro skip: jump on the very first moment in the room: no freeze, no banner, no health bar; Aquatis keeps his first pose for good at full health and never punches, but touching him still costs 10. The jump takes the stick of dynamite floating above the ledge. Jumping a moment later is too late: the introduction runs.
17. The crack: hold Dynamite about 2 s at the ledge's right end and let go: the stick lands on the rock right of the water, its blast reaches the glowing crack behind Aquatis, an earthquake rumbles and the screen shakes for 4 s; an idling Aquatis cowers through it. As it ends a stalactite over him snaps, falls and hits him (the health bar drops a fifth); he reels for about a second, then thrashes for 4 s, punching high and low in turn, with every tentacle. The crack takes no blast for 6 s after a quake, nor while he thrashes or a stalactite falls.
18. Five stalactites kill him, falling in a fixed order (the middle one, second from the left, fourth, leftmost, rightmost): on the fifth he collapses (about 2 s), the gem rises out of him and flies for about 2.5 s to a point above the ledge's right end with the camera following it; Claw stands still until it lands, then takes it and the level ends.
19. Claw's own blows, shots and blasts never take Aquatis's health: they only make him flinch (he cowers after three flinches in a row). A stalactite that falls while he flinches, reels or thrashes, or just after a blow, is wasted; the five then hang again only after Claw loses a life.
20. The dynamite in his room: one stick floats above the ledge (x 37250); once Claw has taken it, a blast anywhere in the room puts a new one there, as long as Claw carries no more sticks than when the last one appeared.
21. Losing a life during the fight: Aquatis is back at full health, every stalactite hangs again, and the fight goes on.
22. The lift: walk right along the hidden floor past the third tentacle and jump so that Claw meets Aquatis's body at the top of the jump (10 and a flinch, held in mid air); on the step after he stands in mid air, press Lift: Claw picks Aquatis up and drops him at once, and he dies in the water; the gem rises out of him with Claw frozen in mid air. Open the help screen (F1) right then, wait about 8 s and close it: the gem is on Claw and the level ends. Without the help screen the gem flies away and Claw falls into the water.
23. The passage under the water (the water skip): from the ledge at the water's edge, throw a stick of dynamite a step before the end, jump left off the end and, while the stick falls, open the help screen (F1) for about 3 s: on the return the stick goes off under Claw and its blast hits him in the air (15 and a flinch), holding him in place. Jump on the step after he stands in mid air and he jumps again, on to the left; a second stick thrown in the air and set off the same way (about 4 s of help) hits him again and gives a second jump. Let go of Left over the water's left end (x 2546 to 2559) and he drops straight through the water onto a hidden floor beneath it, unharmed, and walks under the pool all the way to the boss room; let go further right and the water kills him, further left and he falls past the floor's end. A jump from the hidden floor that comes back down into the water kills him.

## Milestone 3, LEVEL13 early

Failures only, as above: note the line, what happened instead, and where.
Start the thirteenth level with `make run-game ARGS="--install PATH --level 13"`,
or play on from the twelfth level. This section covers what the level
reuses (the red-tail pirates and bear sailors, the dart traps, the ropes,
ferns, pegs and stepping stones), its Chameleons, its water rocks and the
two dynamite throws of the speedrun, and the boss fight with RedTail, his
stakes and his wind.

To try a spot with the tools, `make replay ARGS="--install PATH --level 13
--start X,Y --script ROUTE.csv"` puts Claw at (X, Y) first (the game starts
the level at its start).

1. Chameleons: come within about 500 px of a green lizard on its floor: it runs at Claw, lashes its long tongue from up to about 127 px away (10 and a flinch), or fades out where it stands over about half a second, its picture dissolving into the background.
2. A faded Chameleon is gone for up to 1.5 s, then fades back in somewhere else on its floor (never right where Claw stands) over about half a second. While it fades out, is gone or fades in, nothing hits it and it hurts nothing; it does not fade again for a second after it is back.
3. One hit of anything (sword, pistol, magic, a blast, a thrown object) kills a Chameleon at once, and it cannot be picked up.
4. The lizards near the level's start with no floor limits: the one on the ledge right of the start (x about 1780) is carried off its ledge by a fade and falls to the floor below, where it stays unseen and untouchable; the one on the floor right of it (x about 2430) keeps fading out and back in at the same spot.
5. Water rocks: step onto the flat stone at the water's surface (from the platform at x 3600, walk right onto the rock at x 3744): about a third of a second later a spout throws Claw straight up, 600 px on this rock (250 to 600 px on the others, by rock), whatever the jump key does; landing on it again throws him again. Stepping off before the spout cancels the throw.
6. Dynamite from the bear's block: from the step right of the bear sailor on the first ledges (x 4250), facing right, hold Dynamite about 0.6 s and let go: the stick drops onto the floor below and its blast kills the Chameleon and the red-tail pirate waiting there.
7. Dynamite down the waterfall: from the lip of the waterfall shaft (x 26000), facing right, hold Dynamite about 1.2 s and let go: the stick falls down the shaft and its blast at the foot kills the Chameleon and the red-tail pirate standing there before Claw drops down.
8. A stick thrown straight into a red-tail pirate on this level is usually blocked and destroyed (he parries sword blows half the time); a blast beside him is never blocked.
9. The water: falling into the water anywhere on the level kills Claw at once; he starts again at the last checkpoint passed (at the level's start before the first).
10. The waterfalls: the falling sheets of water and their current move steadily; nothing in them hurts or carries Claw.
11. The toggle pegs in the boss room slide out and in every 1.5 s, in step with each other.

12. RedTail, the introduction: walk right into the warp near the level's end (left of x 33002): Claw arrives on the sand next to the stakes at the left wall and freezes; the camera pans to RedTail on the log ledge, his line, back to Claw, Claw's line, then the name banner drops with a thud and slides off; about 11 s after arriving the fight is on, with his health bar. Half a second later he makes his first move (usually a jump down to the sand and his flare gun).
13. The intro skip: jump at the very first moment on the sand: no freeze, no banner, no health bar; RedTail keeps his idle pose on the ledge at full health and never attacks, jumps or raises the wind for as long as nothing touches him, even with Claw standing right beside him on the ledge; touching him still costs 10. Jumping a moment later is too late: the introduction runs. Walking back onto the warp's spot on the sand later runs the introduction then.
14. His flare gun: on Claw's level he aims and fires a glowing flare that flies straight ahead at about 500 px a second; it costs 15 and bursts where it hits Claw; one that misses is gone at the wall.
15. His slide and kick: on the sand he runs at Claw and slides along the ground into him (10; the slide ends once he is past Claw, or at the wall); close by he kicks instead (10). Touching his body costs 10.
16. His knives: when Claw stands on the other level he throws three knives in a fan up or down at him, straight through the ledge (5 each).
17. The wind: from the ledge, at a Claw on the sand, he raises his arm and a whistling wind blows across the beach for 5 s, grass, leaves and sand flying left: a standing Claw drifts left into the stakes at about 280 px a second, a Claw walking right still loses ground slowly, a jump is carried left; a falling or reeling Claw is not pushed. Held against the stakes Claw loses 20 each time his protection after a hit ends (about every 0.85 s).
18. His jumps: he leaps down to the sand or up onto the ledge to reach Claw's level, and does nothing else in the air.
19. Every third blow he leaps away: each sword blow costs him 5 and makes him flinch, but the third blow that finds him free costs nothing and he leaps to the other level (after an intro skip already the second). Pistol shots and magic he blocks; a blast next to him makes him hop back.
20. His death: twenty sword blows kill him (on the ledge he first jumps down to the sand); he falls, says his last line, and about 2.4 s after he fell the gem rises out of him and flies to a point above the ledge, with Claw frozen where he is (in mid air, if he was jumping) until it arrives; taking it ends the level.
21. Losing a life during the fight: RedTail is back on the ledge at full health, the wind stops, and the fight goes on.

## Milestone 3, LEVEL14 early

Failures only, as above: note the line, what happened instead, and where.
Start the fourteenth level with `make run-game ARGS="--install PATH --level 14"`,
or play on from the thirteenth level. This section covers what the level
reuses (the Chameleons, the fire shooters, the jump switches, the ropes,
pegs, elevators and floor spikes), its tiger guards and its lava: the
fountains, the dripping mouths and the hands that throw lava balls. Omar's
fight and the end of the game come with their own lines below.

To try a spot with the tools, `make replay ARGS="--install PATH --level 14
--start X,Y --script ROUTE.csv"` puts Claw at (X, Y) first (the game starts
the level at its start).

1. Tiger guards: come within about 500 px of an orange or white tiger on its floor: it runs at Claw, and from a few steps away it strikes: a knife combo of two to four slashes (15 each, with a flinch), a lunge (it crouches still for half a second, then dashes about 100 px toward where Claw stood when it crouched, claws out: 20), or a somersault toward Claw that lands about 100 px nearer and ends in one slash. Stepping back during the crouch avoids the lunge.
2. Close in, a tiger nearly always somersaults back once (about 100 px) while it rests between strikes, and when Claw attacks it from in front it sometimes hops back out of reach.
3. The tiger's guard: pistol shots and thrown sticks of dynamite that reach a tiger are gone and it holds its guard for a quarter of a second with a clang, unhurt; a blast next to it, or an object Claw throws at it, only makes it block too. Sword blows, punches and kicks hurt it (three sword blows kill it), and so does magic; but a blow while it lunges, somersaults toward Claw or slashes after that somersault does nothing, and nothing at all touches it during either somersault.
4. A tiger can be picked up and thrown like the pirates; after landing it growls a taunt and strikes back at once. Killed, it drops its loot (every tiger carries some).
5. The lava fountains: columns of lava rise from the lava pools, hold at their full height and fall back, again and again, about every 1.6 s; touching a column costs 10. In the lake with eight fountains they rise in a staggered wave.
6. The lava mouths: stone mouths in the ceilings drip lava without end, a drop swelling under the mouth, then falling to the floor and splashing, about every 1.2 s; the drop costs 10 from when it forms until it splashes.
7. After losing a life, every fountain and mouth near Claw starts its cycle again after its own delay; while it waits, a fountain caught mid-column keeps that column (and its harm) standing, and a mouth caught mid-drip shows an empty mouth but its drop still hurts where it was.
8. The lava hands: walking into the area around a hand (the one at x about 20800, right of the fire shooter, for example) makes it rise out of the lava and throw a lava ball at Claw's side, up to three in a row about 1.2 s apart; then it rests (half a second for that hand, up to 2.5 s for others) before Claw in its area sets it off again.
9. A lava ball flies in an arc trailing sparks and lies where it lands; touching it costs 10 and makes it burst at once; otherwise it bursts a second after the throw, wherever it is, and its small blast hurts Claw (10) and makes a tiger beside it block.
10. The fire shooters: launchers in the walls shoot animated fireballs (5 and a knockback) while Claw stands in front of them.
11. The jump-switch room (the warp down at x about 15380 takes Claw there): the tigers and the two Chameleons pace their ledges, and one that paces onto a hidden switch leaps across the room to another ledge.
12. The lava: falling into the lava anywhere on the level kills Claw (he sinks into it); he starts again at the last checkpoint passed (at the level's start before the first).

<!-- Omar's lines (the warp into his chamber and the fall onto the ladder
top, the staged introduction with no skip by a jump, the end of a life
during it, his ring of fire or ice and the opposite sword, the second
phase in the hall with his energy balls, his death and the gem) follow
here with his fight. -->

## Milestone 3, the end of the game

Failures only, as above: note the line, what happened instead, and where.
Win the fourteenth level (Omar's gem), or, until his fight is in, choose
Credits on the main menu for lines 3 to 6. The original plays its ending
movie after the gem; this game has no movie player yet, so a short black
stands in for it (line 1).

1. Taking the last gem: the level ends as other levels do, the screen stays black for about a second (Escape, Space or Enter end it sooner), then the map screen shows the amulet: the last gem plays into its socket, the amulet is made whole with its sound and Claw says his line; the treasure tally follows as after every level.
2. When the tally is done (or skipped), it fades out, the screen stays black for half a second, and the parchment with the skull and crossed swords unrolls from a narrow strip at the centre to the full screen with its sound.
3. The credits text rises from the bottom over the parchment, white, in a column from near the left edge, at a slow even pace (25 px a second), and the credits music plays and loops; once the last line has left the top, the text starts again from the bottom.
4. Escape, Space, Enter, a game key such as attack or jump, a pad button, or a click anywhere but the top-left corner returns to the main menu at once (no unroll) with the menu music; the arrow keys do nothing. Keys pressed while the parchment unrolls do nothing.
5. A click in the top-left corner (about 100 px square) switches to another tune, shows a line from the developers in white at the centre for 3 s and changes the text's colour every 0.3 s; a second click there, or the tune's end, brings the white text and the credits music back.
6. From the main menu, Credits rolls the menu up and unrolls the parchment, then the same credits run.
7. After the credits the main menu offers every level in New Game and Load Game as before; finishing the game saves nothing new.
