# TAN, explained like you're five

*(The technical version lives in the [README](README.md). This one is for everyone
else.)*

## The problem

You're watching a movie at night. The characters whisper something important, so
you turn the volume up. Thirty seconds later a building explodes and wakes up the
whole house, so you grab the remote and turn it down. Repeat for two hours.

That happens because movies are mixed for theaters, where huge quiet-to-loud
swings feel exciting. On a TV or laptop at home, they're just annoying.

## What TAN does

TAN is like a very fast, very patient friend sitting next to you with a hand on
the volume knob at all times:

- When someone whispers, it nudges the volume up a little.
- When something explodes, it eases the volume down a little.
- The rest of the time, it leaves everything alone.

You hear everything at a comfortable, even level, and you never touch the remote.

## How can it react before the explosion?

It cheats time a tiny bit. TAN holds the sound back for 8 milliseconds - less
than a hundredth of a second, way too short to notice - and uses that head start
to peek at what's coming. If a bang is on the way, the volume is already lowered
by the time it reaches your ears. No flinch, no click.

And when TAN processes a whole file (rather than live audio), it gets to "watch
the movie" once before doing anything. Then it knows about every explosion in
advance and can make every adjustment perfectly smoothly.

## Why doesn't it just make everything the same volume?

That would sound awful - like a robot flattening all the excitement out. TAN
figures out the show's own natural, normal loudness and gently pulls everything
*toward* that. Quiet dialogue comes up to normal, explosions come down to
slightly-above-normal, and the show still feels like itself. It also never
changes the overall volume you chose - your volume knob still means what it
always meant.

One more trick: it measures loudness the way your *ear* hears it, not the way a
computer sees the numbers. A deep rumble and a clear voice can look identical to
a computer but sound completely different to you - TAN uses the same "hearing
model" that the TV and streaming industry uses, so its idea of "too loud" matches
yours.

## Doesn't this already exist?

Sort of. Companies like Dolby sell something similar, but their version needs
special information baked into the movie when it's made, and it only works on
devices that pay to license their technology. TAN does the whole job just by
listening - no secret metadata, no licensed chip, no locked ecosystem - and all
of its code is public, free, and readable by anyone.

## What's it built with?

- **Rust** - a programming language known for being fast and for catching whole
  categories of bugs before the program even runs. Good qualities for something
  that has to process sound perfectly, forever, without glitching.
- **WebAssembly** - a way to run that same code inside a web browser. It's why
  the [demo page](https://bknie1.github.io/TAN/) can normalize your own files, or
  even a playing YouTube tab, right in the page without uploading anything.

## Where's it going?

Today TAN works on files and on browser audio. The plan is for it to eventually
sit quietly inside your computer, phone, or TV and fix *everything* you play,
automatically. The step after that: teaching it to recognize spoken dialogue
specifically, so voices stay clear even when music and effects are trying to
bury them.
