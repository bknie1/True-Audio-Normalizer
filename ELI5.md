# TAN, minus the jargon

The technical version is the [README](README.md). This is the plain one.

## Why I built this

Movies are mixed for theaters. Big quiet-to-loud swings feel great in a theater.
At home you just end up watching the whole thing holding the remote. I have to
turn up the volume for conversations and down for explosions. I like watching
movies, so I thought I would try and solve this problem once and for all.

## What it does

TAN keeps a hand on the volume knob so you don't have to. Whispers get nudged
up. Explosions get eased down. Everything else is left alone. The remote stays
on the couch.

## How it catches an explosion before you hear it

It cheats time a little. TAN holds the sound back 8 milliseconds and uses the
head start to peek at what's coming. That's under a hundredth of a second; you
can't perceive it. The bang shows up already turned down.

Files get a better deal. TAN reads the whole file before touching anything, so
it knows about every explosion in advance. Every adjustment lands smooth.

## Why not make everything the same volume

Because that sounds awful. Flattening the whole mix kills the movie. TAN
figures out the show's own normal loudness and pulls everything gently toward
it. Dialogue comes up to normal. Explosions settle just above normal. Your
volume knob keeps meaning what it always meant.

It also measures loudness the way ears actually work. A deep rumble and a clear
voice can carry identical numbers and sound completely different. TAN uses the
same hearing model the streaming industry standardized on, so its idea of too
loud matches yours.

## Hasn't Dolby done this

Mostly. Their version needs loudness data baked into the movie at the studio,
and it only runs on hardware that licenses their tech. That's a walled garden.
I hate walled gardens.

TAN does the job by listening. No metadata, no licensed hardware. The code is
public, free, and readable.

## What's under the hood

Rust, a language that's fast and catches whole categories of bugs before the
program runs. Good qualities for something that processes sound all day without
glitching.

WebAssembly lets the same code run in a browser. That's why the
[demo page](https://bknie1.github.io/TAN/) can normalize your own files, or a
playing YouTube tab, without uploading anything anywhere.

## Where it's headed

Today it handles files and browser audio. Next it should sit inside your
computer, phone, or TV and fix everything you play automatically. After that I
want it recognizing dialogue specifically, so voices stay clear when the music
tries to bury them.
