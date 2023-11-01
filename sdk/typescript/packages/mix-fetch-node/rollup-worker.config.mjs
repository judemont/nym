/* eslint-disable import/no-extraneous-dependencies */
import commonjs from '@rollup/plugin-commonjs';
import modify from 'rollup-plugin-modify';
import resolve from '@rollup/plugin-node-resolve';
import typescript from '@rollup/plugin-typescript';
import { wasm } from '@rollup/plugin-wasm';

export default {
  input: 'src/worker/index.ts',
  output: {
    dir: 'dist/cjs',
    format: 'cjs',
  },
  onwarn,
  plugins: [
    resolve({
      browser: false,
      preferBuiltins: true,
      extensions: ['.js', '.ts'],
    }),
    commonjs(),
    // TODO: `getObject(...).require` seems to generate a warning on Webpack but with Rollup we get a panic since it can't require.
    // By hard coding the require here, we can workaround that.
    // Reference: https://github.com/rust-random/getrandom/issues/224
    modify({ find: 'getObject(arg0).require(getStringFromWasm0(arg1, arg2));', replace: 'require("crypto");' }),
    wasm({ targetEnv: 'node', maxFileSize: 0, fileName: '[name].wasm' }),
    typescript({
      compilerOptions: {
        outDir: 'dist/cjs',
        declaration: false,
        target: 'es5',
      },
    }),
  ],
};

function onwarn(warning) {
  // fake-indexeddb has a circular dependency that triggers a warning when rolled up
  if (warning.code !== 'CIRCULAR_DEPENDENCY') {
    // eslint-disable-next-line no-console
    console.error(`(!) ${warning.message}`);
  }
}