# Krokodil game

Contains of

* UI 
* Backend 
   * Handles WS connection and connects users into rooms
   
   
## TODO:

* [x] Drawing
* [x] Create a room
    * Index page should list instructions and have a button to create a room
    * When clicking it you are redirected to a page URL.
* [x] Join a room
    * Open a URL
* [x] Pick a word 
    * When another player joins creator picks a word
* [x] Guess a word
    * All other players can guess a word
    * Player take turns in order
* [x] Sync drawing across devices
    * Show drawing for all players 
* [x] How to find a common screen size?
    * I want to find some common canvas size so that it will be displayed the same for all players
    * Alternatively I can simply show how the active player sees canvas
    * Just use a square everywhere
* [x] Figure out what is wrong with my Typescript/preact setup
* [x] Use personal email address in Git
* [x] Test mobile device
* [ ] Tests

* [ ] Get feedback
* [ ] Improve styling and colors
* [x] Show previously guessed words
* [ ] Redirect to https on Heroku
    * https://help.heroku.com/J2R1S4T8/can-heroku-force-an-application-to-use-ssl-tls
* [ ] Add submit drawing button
* [x] Ask for help with a word
    * Atm it is impossible to continue if you don't know the word
* [ ] Fail after 2 failed attempts
* [ ] Allow changing nickname
* [ ] Allow drawing dots


## Build packs

* https://github.com/emk/heroku-buildpack-rust

``` sh
heroku buildpacks:set emk/rust
heroku buildpacks:add heroku/nodejs
```

## References:

* Share: https://developer.mozilla.org/en-US/docs/Web/API/Navigator/share
