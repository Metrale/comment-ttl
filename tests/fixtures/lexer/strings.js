// 2026-09-24: leading line comment
const url = "https://DECOY.example/path"; // trailing after a string with //
const single = 'it // is a DECOY';
const escaped = "quote \" then // DECOY still string";
const tmpl = `outer ${ inner + `nested ${ deep } // DECOY` } // DECOY`;
const multi = `line one
// DECOY line inside a template
${ value }`;
const re = /https?:\/\/DECOY[^/]+/g; // regex literal skipped
const division = total / count / 2; // division is code
/* block comment
   spanning lines */
/**
 * @param {string} a
 * @returns {void}
 */
function f(a) { return /[/]DECOY/.test(a); } // regex after return
  // indented group line 1
  // indented group line 2
// different indent starts a new comment
