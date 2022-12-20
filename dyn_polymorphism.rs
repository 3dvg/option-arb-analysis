// macro_rules! make_alphabet {
//     ($($x:ident),*) => {
//         enum Letter {
//             $(
//                 $x($x),
//             )*
//         }

//         impl AlphabetLetter for Letter {
//             fn some_function(&self) {
//                 match self {
//                     $(
//                         Letter::$x(letter) => letter.some_function(),
//                     )*
//                 }
//             }
//         }
//     };
// }