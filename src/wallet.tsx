import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createAppKit, modal } from "@reown/appkit/react";
import { mainnet, sepolia } from "@reown/appkit/networks";
import { WagmiAdapter } from "@reown/appkit-adapter-wagmi";
import { type ReactNode } from "react";
import { WagmiProvider, createConfig, http } from "wagmi";
import { injected } from "@wagmi/connectors";

const reownProjectId = import.meta.env.VITE_REOWN_PROJECT_ID as string | undefined;
const networks: [typeof mainnet, typeof sepolia] = [mainnet, sepolia];
const queryClient = new QueryClient();
const wagmiAdapter = reownProjectId
  ? new WagmiAdapter({
      networks,
      projectId: reownProjectId,
      ssr: false,
    })
  : null;

const wagmiConfig = wagmiAdapter
  ? wagmiAdapter.wagmiConfig
  : createConfig({
      chains: networks,
      connectors: [injected()],
      transports: {
        [mainnet.id]: http(),
        [sepolia.id]: http(),
      },
    });

if (reownProjectId) {
  createAppKit({
    adapters: wagmiAdapter ? [wagmiAdapter] : [],
    defaultNetwork: mainnet,
    features: {
      analytics: false,
    },
    metadata: {
      name: "Feader",
      description: "Local-first RSS, XPath, and Web3-aware reader",
      url: window.location.origin,
      icons: [],
    },
    networks,
    projectId: reownProjectId,
  });
}

export const isWalletConnectConfigured = Boolean(reownProjectId);

export function openWalletConnectModal(): Promise<void> {
  return modal?.open().then(() => undefined) ?? Promise.resolve();
}

export function WalletProvider({ children }: { children: ReactNode }) {
  return (
    <WagmiProvider config={wagmiConfig}>
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </WagmiProvider>
  );
}
