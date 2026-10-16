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
32. Not on level 1, so not checked here: enemies stepping into goo (they take it like spikes), springboards (Claw's jump height, and enemies on them do not launch), loot carried through a warp, and enemies on moving elevators (they stay put).
