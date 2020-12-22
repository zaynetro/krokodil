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
* [ ] Show how many words each player guessed right and wrong
    * Some basic statistics like PlayerA (5/6)
    * Show previously guessed words
* [x] Sync drawing across devices
    * Show drawing for all players 
* [ ] How to find a common screen size?
    * I want to find some common canvas size so that it will be displayed the same for all players
    * Alternatively I can simply show how the active player sees canvas
* [x] Figure out what is wrong with my Typescript/preact setup
* [x] Use personal email address in Git
* [x] Test mobile device
* [ ] Tests


References:

* Share: https://developer.mozilla.org/en-US/docs/Web/API/Navigator/share
