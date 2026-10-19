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
