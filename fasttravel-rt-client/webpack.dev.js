const path = require('path');
const webpack = require('webpack');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const wasmPackPlugin = require('@wasm-tool/wasm-pack-plugin');


module.exports = {
    mode: 'development',
    entry: ["./test-apps/js/index.js"],
    output: {
        filename: '[name].bundle.js',
        path: path.resolve(__dirname, 'dist'),
    },
    resolve: {
        // Add '.ts' and '.tsx' as resolvable extensions.
        extensions: ["", ".webpack.js", ".web.js", ".ts", ".tsx", ".js"],
    },
    module: {
        rules: [
            // All files with a '.ts' or '.tsx' extension will be handled by 'ts-loader'.
            { test: /\.tsx?$/, loader: "ts-loader" },
            // All output '.js' files will have any sourcemaps re-processed by 'source-map-loader'.
            { test: /\.js$/, loader: "source-map-loader" },
        ]
    },
    devServer: {
        static: {
            directory: path.join(__dirname, 'dist'),
        },
        port: 3033,
        host: '0.0.0.0',
    },
    // Enable sourcemaps for debugging webpack's output.
    devtool: "source-map",
    plugins: [
        new HtmlWebpackPlugin(),
        new wasmPackPlugin({
            crateDirectory: path.resolve(__dirname, "."),
            outDir: path.resolve(__dirname, "pkg"),
            outName: "fasttravel_rt_client_private",
        })],
    experiments: {
        asyncWebAssembly: true,
        topLevelAwait: true,
    },
}
