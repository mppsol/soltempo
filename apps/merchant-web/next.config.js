/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Transpile workspace package so its TS source resolves directly.
  transpilePackages: ["@soltempo/types"],
  webpack: (config) => {
    // viem and @solana/web3.js use top-level await + ESM exports.
    config.experiments = { ...config.experiments, topLevelAwait: true };
    return config;
  },
};
export default nextConfig;
