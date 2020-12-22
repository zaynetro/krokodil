import resolve from '@rollup/plugin-node-resolve';
import postcss from 'rollup-plugin-postcss';
import typescript from '@rollup/plugin-typescript';

export default {
  input: 'src/main.tsx',
  output: {
    dir: 'dist',
    format: 'es',
    sourcemap: true
  },
  cache: true,
  plugins: [
    typescript(),
    postcss({
      extract: true,
      modules: true,
    }),
    // Include external dependencies in the bundle
    resolve(),
  ]
};
