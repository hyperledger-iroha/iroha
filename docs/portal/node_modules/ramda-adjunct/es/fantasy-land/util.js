import { useWith, curry, curryN, equals, path, either, complement } from 'ramda';

// type :: Monad a => a -> String
export var type = either(path(['@@type']), path(['constructor', '@@type']));

// typeEquals :: Monad a => String -> a -> Boolean
export var typeEquals = curry(function (typeIdent, monad) {
  return type(monad) === typeIdent;
});

// isSameType :: (Monad a, Monad b) => a -> b -> Boolean
export var isSameType = curryN(2, useWith(equals, [type, type]));

// isNotSameType :: (Monad a, Monad b) => a -> b -> Boolean
export var isNotSameType = complement(isSameType);