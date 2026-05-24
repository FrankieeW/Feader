import { useEffect, useMemo, useState } from "react";
import type { CSSProperties, FormEvent, KeyboardEvent, PointerEvent } from "react";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { useAccount, useConnect, useDisconnect, useSignMessage } from "wagmi";
import DOMPurify from "dompurify";
import {
  isWalletConnectConfigured,
  openWalletConnectModal,
} from "./wallet";
import "./App.css";

type ReaderShortcutEvent = {
  altKey: boolean;
  ctrlKey: boolean;
  key: string;
  metaKey: boolean;
  target: EventTarget | null;
  preventDefault: () => void;
};

type Source = {
  id: number;
  kind: string;
  title: string;
  url: string;
  category?: string | null;
  configJson?: string | null;
  enabled: boolean;
  createdAt: string;
  lastFetchedAt?: string | null;
  lastError?: string | null;
  articleCount: number;
  unreadCount: number;
};

type Article = {
  id: number;
  sourceId: number;
  sourceTitle: string;
  externalId?: string | null;
  title: string;
  url: string;
  canonicalUrl?: string | null;
  summary?: string | null;
  contentHtml?: string | null;
  contentText?: string | null;
  author?: string | null;
  publishedAt?: string | null;
  imageUrl?: string | null;
  tagsJson?: string | null;
  read: boolean;
  saved: boolean;
  createdAt: string;
  updatedAt: string;
};

type ArticleFilter = {
  sourceId?: number;
  unreadOnly?: boolean;
  savedOnly?: boolean;
};

type SourceRefreshResult = {
  sourceId: number;
  ok: boolean;
  articleCount: number;
  error?: string | null;
};

type FilterMode = "all" | "unread" | "saved";
type SourceInputMode = "rss" | "xpath";
type ThemeMode = "light" | "dark" | "system";
type ViewMode = "reader" | "sources" | "settings";
type EntryLayout = "list" | "card";
type ReaderTypography = "system" | "serif" | "large";
type ReaderView = "none" | "preview" | "immersive";
type PaneKey = "sidebar" | "timeline";

type PaneWidths = {
  sidebar: number;
  timeline: number;
};

type XPathSelectors = {
  items: string;
  title: string;
  url: string;
  summary?: string;
  publishedAt?: string;
  author?: string;
  content?: string;
  image?: string;
  nextPage?: string;
};

type XPathSourceSuggestion = {
  title?: string | null;
  selectors: XPathSelectors;
};

type ParsedArticle = {
  title: string;
  url: string;
  summary?: string | null;
  publishedAt?: string | null;
  author?: string | null;
  contentText?: string | null;
  imageUrl?: string | null;
};

type XPathFieldDiagnostic = {
  field: string;
  label: string;
  required: boolean;
  expression?: string | null;
  status: string;
  message: string;
  sample?: string | null;
};

type XPathPreview = {
  articles: ParsedArticle[];
  diagnostics: XPathFieldDiagnostic[];
  nextPageUrl?: string | null;
};

type AiProvider = "anthropic" | "openai";

type AiSettings = {
  provider: AiProvider;
  baseUrl: string;
  model: string;
  enabled: boolean;
  apiKeySet: boolean;
  apiKeyReference?: string | null;
  updatedAt: string;
};

type WalletLoginChallenge = {
  nonce: string;
  domain: string;
  uri: string;
  statement: string;
  issuedAt: string;
  expiresAt: string;
};

type WalletSession = {
  address: string;
  chainId: number;
  signedInAt: string;
  expiresAt?: string | null;
};

const defaultXPathSelectors: XPathSelectors = {
  items: "//article",
  title: ".//h2/a/text()",
  url: ".//h2/a/@href",
  summary: ".//p/text()",
  publishedAt: ".//time/@datetime",
  author: "",
  content: ".",
  image: ".//img/@src",
  nextPage: "",
};

const xpathPresets: Record<string, XPathSelectors> = {
  "Generic blog": {
    items: "//article",
    title: ".//h2/a | .//h3/a",
    url: ".//h2/a/@href | .//h3/a/@href",
    summary: ".//p",
    publishedAt: ".//time/@datetime",
    author: "",
    content: ".//section",
    image: ".//img/@src",
    nextPage: "//a[@rel='next']/@href",
  },
  "Listing + links": {
    items: "//li[.//a]",
    title: ".//a",
    url: ".//a/@href",
    summary: "",
    publishedAt: "",
    author: "",
    content: "",
    image: ".//img/@src",
    nextPage: "",
  },
};

const themeStorageKey = "feader.theme";
const entryLayoutStorageKey = "feader.entryLayout";
const paneStorageKey = "feader.paneWidths";
const readerTypographyStorageKey = "feader.readerTypography";
const feedGroupStorageKey = "feader.feedGroups";
const categoryDatalistId = "feader-category-options";
const builtInTestFeedUrl = "https://www.appinn.com/feed/";
const aiDocsUrl = "https://github.com/FrankieeW/Feader/blob/main/docs/ai-configuration.md";
const defaultPaneWidths: PaneWidths = {
  sidebar: 240,
  timeline: 520,
};
const paneBounds: Record<PaneKey, { min: number; max: number }> = {
  sidebar: { min: 220, max: 300 },
  timeline: { min: 360, max: 620 },
};

const defaultAiSettings: AiSettings = {
  provider: "openai",
  baseUrl: "",
  model: "",
  enabled: false,
  apiKeySet: false,
  apiKeyReference: null,
  updatedAt: "",
};

const testModeSources: Source[] = [
  {
    id: 1,
    kind: "rss",
    title: "小众软件",
    url: builtInTestFeedUrl,
    category: "News",
    enabled: true,
    createdAt: "2026-05-23T08:00:00.000Z",
    lastFetchedAt: "2026-05-23T08:00:00.000Z",
    lastError: null,
    articleCount: 5,
    unreadCount: 4,
  },
];

const testModeArticles: Article[] = [
  {
    id: 1,
    sourceId: 1,
    sourceTitle: "小众软件",
    externalId: "appinn-test-1",
    title: "小众软件 RSS 测试源已接入 Feader",
    url: "https://www.appinn.com/",
    canonicalUrl: "https://www.appinn.com/",
    summary: "内置测试模式使用 https://www.appinn.com/feed/ 作为默认数据源。",
    contentText:
      "这是 Feader 的内置测试模式数据。桌面端会继续通过 Tauri 命令访问本地 SQLite 与真实 RSS；浏览器预览则展示小众软件 feed 的固定测试源，避免没有 Tauri 后端时页面为空。",
    author: "Feader",
    publishedAt: "2026-05-23T08:00:00.000Z",
    read: false,
    saved: true,
    createdAt: "2026-05-23T08:00:00.000Z",
    updatedAt: "2026-05-23T08:00:00.000Z",
  },
  {
    id: 2,
    sourceId: 1,
    sourceTitle: "小众软件",
    externalId: "appinn-test-2",
    title: "用 RSS 跟踪软件更新与工具推荐",
    url: "https://www.appinn.com/feed/",
    canonicalUrl: "https://www.appinn.com/feed/",
    summary: "Feed 地址会显示在来源详情中，便于验证添加源、筛选、已读和收藏状态。",
    contentText:
      "测试数据保留真实 feed URL，交互状态在当前浏览器会话内更新。刷新按钮会模拟成功刷新，不会发起外部网络请求。",
    author: "Feader",
    publishedAt: "2026-05-22T09:15:00.000Z",
    read: false,
    saved: false,
    createdAt: "2026-05-22T09:15:00.000Z",
    updatedAt: "2026-05-22T09:15:00.000Z",
  },
  {
    id: 3,
    sourceId: 1,
    sourceTitle: "小众软件",
    externalId: "appinn-test-3",
    title: "轻量阅读器需要清楚的来源健康状态",
    url: "https://www.appinn.com/",
    canonicalUrl: "https://www.appinn.com/",
    summary: "来源、文章数量、未读数量和最近刷新时间都来自同一组内置测试数据。",
    contentText:
      "这条测试文章用于验证详情栏和阅读面板。测试模式与真实 Tauri 命令路径隔离，因此不会污染本地数据库。",
    author: "Feader",
    publishedAt: "2026-05-21T11:30:00.000Z",
    read: true,
    saved: false,
    createdAt: "2026-05-21T11:30:00.000Z",
    updatedAt: "2026-05-21T11:30:00.000Z",
  },
  {
    id: 4,
    sourceId: 1,
    sourceTitle: "小众软件",
    externalId: "appinn-test-4",
    title: "在浏览器中预览 Feader 的三栏阅读工作台",
    url: "https://www.appinn.com/",
    canonicalUrl: "https://www.appinn.com/",
    summary: "这条未读文章用于检查列表密度、选中态、按钮悬停态和阅读面板排版。",
    contentText:
      "测试模式的目标是让 Vite 预览能直接展示完整界面，同时保持桌面应用真实数据路径不变。",
    author: "Feader",
    publishedAt: "2026-05-20T13:45:00.000Z",
    read: false,
    saved: false,
    createdAt: "2026-05-20T13:45:00.000Z",
    updatedAt: "2026-05-20T13:45:00.000Z",
  },
  {
    id: 5,
    sourceId: 1,
    sourceTitle: "小众软件",
    externalId: "appinn-test-5",
    title: "Appinn feed 作为默认验证入口",
    url: builtInTestFeedUrl,
    canonicalUrl: builtInTestFeedUrl,
    summary: "默认源 URL 精确设置为 https://www.appinn.com/feed/。",
    contentText:
      "来源面板中的 URL 字段会显示该地址。添加源表单也默认以 RSS/Atom 流程为主，保持真实使用路径一致。",
    author: "Feader",
    publishedAt: "2026-05-19T15:00:00.000Z",
    read: false,
    saved: false,
    createdAt: "2026-05-19T15:00:00.000Z",
    updatedAt: "2026-05-19T15:00:00.000Z",
  },
];

let testModeSourceState = testModeSources.map((source) => ({ ...source }));
let testModeArticleState = testModeArticles.map((article) => ({ ...article }));
let testModeAiSettings: AiSettings = { ...defaultAiSettings };

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriRuntime()) {
    return tauriInvoke<T>(command, args);
  }
  return testModeInvoke<T>(command, args);
}

function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

async function testModeInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  switch (command) {
    case "list_sources":
      return syncTestModeSources() as T;
    case "list_articles":
      return filterTestModeArticles(args?.filter as ArticleFilter | null | undefined) as T;
    case "refresh_source":
      touchTestModeSource(Number(args?.sourceId));
      return filterTestModeArticles({ sourceId: Number(args?.sourceId) }) as T;
    case "refresh_all_sources":
      testModeSourceState = testModeSourceState.map((source) => ({
        ...source,
        lastFetchedAt: new Date().toISOString(),
        lastError: null,
      }));
      return testModeSourceState.map((source) => ({
        sourceId: source.id,
        ok: true,
        articleCount: source.articleCount,
        error: null,
      })) as T;
    case "mark_article_read":
      setTestModeArticleState(Number(args?.articleId), { read: Boolean(args?.read) });
      return undefined as T;
    case "save_article":
      setTestModeArticleState(Number(args?.articleId), { saved: Boolean(args?.saved) });
      return undefined as T;
    case "mark_articles_read": {
      const sourceId = typeof args?.sourceId === "number" ? args.sourceId : undefined;
      let changed = 0;
      testModeArticleState = testModeArticleState.map((article) => {
        if (sourceId && article.sourceId !== sourceId) {
          return article;
        }
        if (article.read === Boolean(args?.read)) {
          return article;
        }
        changed += 1;
        return { ...article, read: Boolean(args?.read), updatedAt: new Date().toISOString() };
      });
      syncTestModeSources();
      return changed as T;
    }
    case "add_source": {
      const request = args?.request as { url?: string; title?: string } | undefined;
      return upsertTestModeSource(request?.url, request?.title) as T;
    }
    case "create_wallet_login_challenge":
      return {
        nonce: "testnonce1",
        domain: "localhost:1420",
        uri: "http://localhost:1420",
        statement: "Sign in to Feader with your Ethereum wallet.",
        issuedAt: new Date().toISOString(),
        expiresAt: new Date(Date.now() + 600_000).toISOString(),
      } as T;
    case "get_wallet_session":
      return null as T;
    case "verify_wallet_login":
      return {
        address: "0x0000000000000000000000000000000000000000",
        chainId: 1,
        signedInAt: new Date().toISOString(),
        expiresAt: null,
      } as T;
    case "disconnect_wallet_login":
      return undefined as T;
    case "get_ai_settings":
      return testModeAiSettings as T;
    case "set_ai_settings": {
      const input = args?.input as
        | {
            provider?: AiProvider;
            baseUrl?: string;
            model?: string;
            enabled?: boolean;
            apiKey?: string | null;
          }
        | undefined;
      const key = typeof input?.apiKey === "string" ? input.apiKey.trim() : "";
      const hadKey = testModeAiSettings.apiKeySet;
      testModeAiSettings = {
        provider: input?.provider ?? testModeAiSettings.provider,
        baseUrl: input?.baseUrl ?? "",
        model: input?.model ?? "",
        enabled: Boolean(input?.enabled),
        apiKeySet: key.length > 0 ? true : hadKey,
        apiKeyReference: key.startsWith("$")
          ? key
          : key.length > 0
            ? null
            : testModeAiSettings.apiKeyReference,
        updatedAt: new Date().toISOString(),
      };
      return testModeAiSettings as T;
    }
    case "update_source_title": {
      const request = args?.request as { sourceId?: number; title?: string } | undefined;
      const sourceId = Number(request?.sourceId);
      testModeSourceState = testModeSourceState.map((source) =>
        source.id === sourceId ? { ...source, title: request?.title || source.title } : source,
      );
      return testModeSourceState.find((source) => source.id === sourceId) as T;
    }
    case "delete_source": {
      const sourceId = Number(args?.sourceId);
      testModeSourceState = testModeSourceState.filter((source) => source.id !== sourceId);
      testModeArticleState = testModeArticleState.filter(
        (article) => article.sourceId !== sourceId,
      );
      return undefined as T;
    }
    case "preview_xpath_source":
      return {
        articles: testModeArticleState
          .slice(0, 3)
          .map(({ title, url, summary, publishedAt, author, contentText, imageUrl }) => ({
            title,
            url,
            summary,
            publishedAt,
            author,
            contentText,
            imageUrl,
          })),
        diagnostics: [
          {
            field: "items",
            label: "Items",
            required: true,
            expression: "//article",
            status: "ok",
            message: "Item nodes matched.",
            sample: "3",
          },
          {
            field: "title",
            label: "Title",
            required: true,
            expression: ".//h2/a/text()",
            status: "ok",
            message: "Values found in preview items.",
            sample: testModeArticleState[0]?.title,
          },
          {
            field: "url",
            label: "URL",
            required: true,
            expression: ".//h2/a/@href",
            status: "ok",
            message: "Values found in preview items.",
            sample: testModeArticleState[0]?.url,
          },
        ],
        nextPageUrl: null,
      } as T;
    case "add_xpath_source":
      throw new Error("XPath test mode is read-only. Use the Tauri app to validate XPath sources.");
    case "suggest_xpath_source":
      throw new Error("AI suggestions require the Tauri app.");
    case "set_source_category": {
      const sourceId = Number(args?.sourceId);
      const rawCategory = typeof args?.category === "string" ? args.category.trim() : "";
      const category = rawCategory.length > 0 ? rawCategory : null;
      testModeSourceState = testModeSourceState.map((source) =>
        source.id === sourceId ? { ...source, category } : source,
      );
      return testModeSourceState.find((source) => source.id === sourceId) as T;
    }
    default:
      throw new Error(`Test mode command '${command}' is not implemented.`);
  }
}

function syncTestModeSources(): Source[] {
  testModeSourceState = testModeSourceState.map((source) => {
    const articles = testModeArticleState.filter((article) => article.sourceId === source.id);
    return {
      ...source,
      articleCount: articles.length,
      unreadCount: articles.filter((article) => !article.read).length,
    };
  });
  return testModeSourceState.map((source) => ({ ...source }));
}

function filterTestModeArticles(filter?: ArticleFilter | null): Article[] {
  return testModeArticleState
    .filter((article) => !filter?.sourceId || article.sourceId === filter.sourceId)
    .filter((article) => !filter?.unreadOnly || !article.read)
    .filter((article) => !filter?.savedOnly || article.saved)
    .map((article) => ({ ...article }));
}

function touchTestModeSource(sourceId: number): void {
  testModeSourceState = testModeSourceState.map((source) =>
    source.id === sourceId ? { ...source, lastFetchedAt: new Date().toISOString(), lastError: null } : source,
  );
}

function setTestModeArticleState(articleId: number, patch: Partial<Article>): void {
  testModeArticleState = testModeArticleState.map((article) =>
    article.id === articleId ? { ...article, ...patch, updatedAt: new Date().toISOString() } : article,
  );
  syncTestModeSources();
}

function upsertTestModeSource(url = builtInTestFeedUrl, title = "小众软件"): Source {
  const trimmedUrl = url.trim() || builtInTestFeedUrl;
  const existing = testModeSourceState.find((source) => source.url === trimmedUrl);
  if (existing) {
    return existing;
  }

  const source: Source = {
    id: Math.max(0, ...testModeSourceState.map((item) => item.id)) + 1,
    kind: "rss",
    title: title.trim() || trimmedUrl,
    url: trimmedUrl,
    category: null,
    enabled: true,
    createdAt: new Date().toISOString(),
    lastFetchedAt: new Date().toISOString(),
    lastError: null,
    articleCount: 0,
    unreadCount: 0,
  };
  testModeSourceState = [...testModeSourceState, source];
  return source;
}

const uncategorizedLabel = "Uncategorized";

function groupSourcesByCategory(sources: Source[]): { category: string; sources: Source[] }[] {
  const groups = new Map<string, Source[]>();
  for (const source of sources) {
    const key = source.category?.trim() ? source.category.trim() : uncategorizedLabel;
    const bucket = groups.get(key) ?? [];
    groups.set(key, [...bucket, source]);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => {
      if (a === uncategorizedLabel) return 1;
      if (b === uncategorizedLabel) return -1;
      return a.localeCompare(b);
    })
    .map(([category, sources]) => ({ category, sources }));
}

function readInitialCollapsedGroups(): Record<string, boolean> {
  const stored = localStorage.getItem(feedGroupStorageKey);
  if (!stored) {
    return {};
  }
  try {
    return JSON.parse(stored) as Record<string, boolean>;
  } catch {
    return {};
  }
}

function App() {
  const { address: walletAddress, chainId, isConnected } = useAccount();
  const { connectAsync, connectors } = useConnect();
  const { disconnectAsync } = useDisconnect();
  const { signMessageAsync } = useSignMessage();
  const [sources, setSources] = useState<Source[]>([]);
  const [articles, setArticles] = useState<Article[]>([]);
  const [selectedSourceId, setSelectedSourceId] = useState<number | undefined>();
  const [selectedArticleId, setSelectedArticleId] = useState<number | undefined>();
  const [readerView, setReaderView] = useState<ReaderView>("none");
  const [filterMode, setFilterMode] = useState<FilterMode>("all");
  const [sourceInputMode, setSourceInputMode] = useState<SourceInputMode>("rss");
  const [activeView, setActiveView] = useState<ViewMode>("reader");
  const [showSourceComposer, setShowSourceComposer] = useState(false);
  const [themeMode, setThemeMode] = useState<ThemeMode>(() => readInitialThemeMode());
  const [entryLayout, setEntryLayout] = useState<EntryLayout>(() => readInitialEntryLayout());
  const [readerTypography, setReaderTypography] = useState<ReaderTypography>(() =>
    readInitialReaderTypography(),
  );
  const [paneWidths, setPaneWidths] = useState<PaneWidths>(() => readInitialPaneWidths());
  const [feedUrl, setFeedUrl] = useState("");
  const [xpathTitle, setXPathTitle] = useState("");
  const [xpathSelectors, setXPathSelectors] = useState<XPathSelectors>(defaultXPathSelectors);
  const [xpathPreview, setXPathPreview] = useState<XPathPreview | null>(null);
  const [xpathStatus, setXPathStatus] = useState<string | null>(null);
  const [aiSettings, setAiSettings] = useState<AiSettings>(defaultAiSettings);
  const [walletSession, setWalletSession] = useState<WalletSession | null>(null);
  const [status, setStatus] = useState("Ready");
  const [isBusy, setIsBusy] = useState(false);

  const selectedSource = useMemo(
    () => sources.find((source) => source.id === selectedSourceId),
    [selectedSourceId, sources],
  );
  const selectedArticle = useMemo(
    () => articles.find((article) => article.id === selectedArticleId) ?? articles[0],
    [articles, selectedArticleId],
  );
  const unreadCount = sources.reduce((total, source) => total + source.unreadCount, 0);
  const failedSourceCount = sources.filter((source) => source.lastError).length;
  const selectedSourceHealth = selectedSource ? sourceHealth(selectedSource) : "Mixed";
  const categoryOptions = useMemo(
    () =>
      [
        ...new Set(
          sources
            .map((source) => source.category?.trim())
            .filter((value): value is string => Boolean(value)),
        ),
      ].sort(),
    [sources],
  );

  useEffect(() => {
    void loadData();
    void loadWalletSession();
    void loadAiSettings();
  }, []);

  useEffect(() => {
    applyThemeMode(themeMode);
    localStorage.setItem(themeStorageKey, themeMode);

    if (themeMode !== "system") {
      return;
    }

    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => applyThemeMode("system");
    media.addEventListener("change", handleChange);
    return () => media.removeEventListener("change", handleChange);
  }, [themeMode]);

  useEffect(() => {
    localStorage.setItem(entryLayoutStorageKey, entryLayout);
  }, [entryLayout]);

  useEffect(() => {
    localStorage.setItem(readerTypographyStorageKey, readerTypography);
  }, [readerTypography]);

  useEffect(() => {
    localStorage.setItem(paneStorageKey, JSON.stringify(paneWidths));
  }, [paneWidths]);

  useEffect(() => {
    if (!selectedArticle) {
      setReaderView("none");
    }
  }, [selectedArticle]);

  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>(() =>
    readInitialCollapsedGroups(),
  );

  useEffect(() => {
    localStorage.setItem(feedGroupStorageKey, JSON.stringify(collapsedGroups));
  }, [collapsedGroups]);

  const sourceGroups = useMemo(() => groupSourcesByCategory(sources), [sources]);

  async function loadData(
    sourceId = selectedSourceId,
    mode = filterMode,
    nextSelectedArticleId = selectedArticleId,
  ): Promise<void> {
    const filter = buildArticleFilter(sourceId, mode);
    const [nextSources, nextArticles] = await Promise.all([
      invoke<Source[]>("list_sources"),
      invoke<Article[]>("list_articles", { filter }),
    ]);
    setSources(nextSources);
    setArticles(nextArticles);
    setSelectedArticleId(resolveSelectedArticleId(nextArticles, nextSelectedArticleId));
  }

  async function loadWalletSession(): Promise<void> {
    const session = await invoke<WalletSession | null>("get_wallet_session");
    setWalletSession(session);
  }

  async function loadAiSettings(): Promise<void> {
    const settings = await invoke<AiSettings>("get_ai_settings");
    setAiSettings(settings);
  }

  async function handleSaveAiSettings(input: {
    provider: AiProvider;
    baseUrl: string;
    model: string;
    enabled: boolean;
    apiKey?: string;
  }): Promise<void> {
    await runTask("Saving AI settings", async () => {
      const next = await invoke<AiSettings>("set_ai_settings", { input });
      setAiSettings(next);
      setStatus("AI settings saved");
    });
  }

  async function handleConnectWallet(): Promise<void> {
    await runTask("Connecting wallet", async () => {
      if (isWalletConnectConfigured) {
        await openWalletConnectModal();
        setStatus("WalletConnect opened. Sign in after connecting your wallet.");
        return;
      }

      const connector = connectors.find((item) => item.id === "injected") ?? connectors[0];
      if (!connector) {
        throw new Error("No injected wallet is available. Set VITE_REOWN_PROJECT_ID to enable WalletConnect QR login.");
      }
      await connectAsync({ connector });
      setStatus("Wallet connected. Sign in to verify ownership.");
    });
  }

  async function handleWalletSignIn(): Promise<void> {
    if (!walletAddress || !chainId) {
      setStatus("Connect an EVM wallet before signing in.");
      return;
    }

    await runTask("Signing in wallet", async () => {
      const challenge = await invoke<WalletLoginChallenge>("create_wallet_login_challenge", {
        request: {
          domain: window.location.host,
          uri: window.location.origin,
        },
      });
      const message = buildSiweMessage(challenge, walletAddress, chainId);
      const signature = await signMessageAsync({ message });
      const session = await invoke<WalletSession>("verify_wallet_login", {
        request: { message, signature },
      });
      setWalletSession(session);
      setStatus(`Signed in as ${shortAddress(session.address)}`);
    });
  }

  async function handleWalletDisconnect(): Promise<void> {
    await runTask("Disconnecting wallet", async () => {
      await invoke<void>("disconnect_wallet_login");
      await disconnectAsync().catch(() => undefined);
      setWalletSession(null);
      setStatus("Wallet disconnected.");
    });
  }

  async function handleAddFeed(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const url = feedUrl.trim();
    if (!url) {
      const message = "Enter a feed URL first.";
      setStatus(message);
      if (sourceInputMode === "xpath") {
        setXPathStatus(message);
      }
      return;
    }

    if (sourceInputMode === "xpath") {
      setXPathStatus("Confirming XPath source...");
    }
    await runTask("Adding feed", async () => {
      const source =
        sourceInputMode === "rss"
          ? await invoke<Source>("add_source", { request: { url } })
          : await invoke<Source>("add_xpath_source", {
              request: {
                url,
                title: xpathTitle,
                selectors: compactXPathSelectors(xpathSelectors),
              },
            });
      setFeedUrl("");
      setXPathTitle("");
      setXPathPreview(null);
      setXPathStatus(null);
      setShowSourceComposer(false);
      setSelectedSourceId(source.id);
      setFilterMode("all");
      await loadData(source.id, "all", undefined);
      setStatus(`Added ${source.title}`);
    }, sourceInputMode === "xpath" ? setXPathStatus : undefined);
  }

  async function handlePreviewXPath(): Promise<void> {
    const url = feedUrl.trim();
    if (!url) {
      const message = "Enter a page URL first.";
      setStatus(message);
      setXPathStatus(message);
      return;
    }

    setXPathStatus("Previewing XPath selectors...");
    await runTask("Previewing XPath", async () => {
      const preview = await invoke<XPathPreview>("preview_xpath_source", {
        request: {
          url,
          selectors: compactXPathSelectors(xpathSelectors),
        },
      });
      setXPathPreview(preview);
      const message = `Preview extracted ${preview.articles.length} articles`;
      setStatus(message);
      setXPathStatus(message);
    }, setXPathStatus);
  }

  async function handleSuggestXPath(): Promise<void> {
    const url = feedUrl.trim();
    if (!url) {
      const message = "Enter a page URL first.";
      setStatus(message);
      setXPathStatus(message);
      return;
    }

    setXPathStatus("Suggesting selectors with AI...");
    await runTask("Suggesting selectors", async () => {
      const suggested = await invoke<XPathSourceSuggestion>("suggest_xpath_source", { url });
      const nextSelectors = normalizeXPathSelectorsForForm(suggested.selectors);
      const suggestedTitle = suggested.title?.trim();
      if (suggestedTitle) {
        setXPathTitle(suggestedTitle);
      }
      setXPathSelectors(nextSelectors);
      setXPathPreview(null);
      const titleSummary = suggestedTitle ? `Source title: ${suggestedTitle}; ` : "";
      const message = `AI suggested ${titleSummary}Items: ${nextSelectors.items}; Title: ${nextSelectors.title}; URL: ${nextSelectors.url}. Run Preview to validate.`;
      setStatus(message);
      setXPathStatus(message);
    }, setXPathStatus);
  }

  async function handleSelectSource(sourceId?: number): Promise<void> {
    setSelectedSourceId(sourceId);
    await loadData(sourceId, filterMode, undefined);
  }

  async function handleSetFilter(mode: FilterMode): Promise<void> {
    setFilterMode(mode);
    await loadData(selectedSourceId, mode, undefined);
  }

  async function handleRefreshSource(sourceId: number): Promise<void> {
    await runTask("Refreshing feed", async () => {
      await invoke<Article[]>("refresh_source", { sourceId });
      await loadData(selectedSourceId, filterMode, selectedArticleId);
      setStatus("Feed refreshed");
    });
  }

  async function handleRefreshAll(): Promise<void> {
    await runTask("Refreshing all feeds", async () => {
      const results = await invoke<SourceRefreshResult[]>("refresh_all_sources");
      await loadData(selectedSourceId, filterMode, selectedArticleId);
      const failed = results.filter((result) => !result.ok).length;
      setStatus(
        failed === 0
          ? `Refreshed ${results.length} feeds`
          : `Refreshed ${results.length - failed}; ${failed} failed`,
      );
    });
  }

  async function handleSetCategory(sourceId: number, category: string): Promise<void> {
    await runTask("Updating category", async () => {
      await invoke<Source>("set_source_category", { sourceId, category });
      await loadData(selectedSourceId, filterMode, selectedArticleId);
      setStatus("Category updated");
    });
  }

  async function handleRenameSourceId(sourceId: number, title: string): Promise<void> {
    const nextTitle = title.trim();
    if (!nextTitle) {
      return;
    }
    await runTask("Renaming feed", async () => {
      await invoke<Source>("update_source_title", {
        request: { sourceId, title: nextTitle },
      });
      await loadData(selectedSourceId, filterMode, selectedArticleId);
      setStatus("Feed renamed");
    });
  }

  async function handleDeleteSourceId(sourceId: number, title: string): Promise<void> {
    const confirmed = window.confirm(`Delete "${title}" and its articles?`);
    if (!confirmed) {
      return;
    }
    await runTask("Deleting feed", async () => {
      await invoke("delete_source", { sourceId });
      if (selectedSourceId === sourceId) {
        setSelectedSourceId(undefined);
        setSelectedArticleId(undefined);
        await loadData(undefined, filterMode, undefined);
      } else {
        await loadData(selectedSourceId, filterMode, selectedArticleId);
      }
      setStatus("Feed deleted");
    });
  }

  async function handleMarkAllRead(): Promise<void> {
    await runTask("Marking articles read", async () => {
      const changed = await invoke<number>("mark_articles_read", {
        sourceId: selectedSourceId ?? null,
        read: true,
      });
      await loadData(selectedSourceId, filterMode, selectedArticleId);
      setStatus(`Marked ${changed} articles read`);
    });
  }

  async function handleToggleRead(article: Article): Promise<void> {
    await invoke("mark_article_read", {
      articleId: article.id,
      read: !article.read,
    });
    await loadData(selectedSourceId, filterMode, article.id);
  }

  async function handleToggleSaved(article: Article): Promise<void> {
    await invoke("save_article", {
      articleId: article.id,
      saved: !article.saved,
    });
    await loadData(selectedSourceId, filterMode, article.id);
  }

  function handleArticleKeyDown(event: KeyboardEvent<HTMLElement>, articleId: number): void {
    if (event.key !== "Enter") {
      return;
    }

    event.preventDefault();
    setSelectedArticleId(articleId);
  }

  function handleAppKeyDown(event: ReaderShortcutEvent): void {
    if (activeView !== "reader" || isTextInputTarget(event.target)) {
      return;
    }

    if (event.key === "Escape") {
      if (readerView !== "none") {
        event.preventDefault();
        setReaderView("none");
      }
      return;
    }

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      selectRelativeArticle(event.key === "ArrowDown" ? 1 : -1);
      return;
    }

    if (!selectedArticle || event.metaKey || event.ctrlKey || event.altKey) {
      return;
    }

    if (event.key === " ") {
      event.preventDefault();
      setReaderView((current) => (current === "preview" ? "none" : "preview"));
      return;
    }

    if (event.key.toLowerCase() === "r") {
      event.preventDefault();
      void handleToggleRead(selectedArticle);
    }

    if (event.key.toLowerCase() === "s") {
      event.preventDefault();
      void handleToggleSaved(selectedArticle);
    }
  }

  useEffect(() => {
    function handleDocumentKeyDown(event: globalThis.KeyboardEvent): void {
      if (event.defaultPrevented) {
        return;
      }

      handleAppKeyDown(event);
    }

    document.addEventListener("keydown", handleDocumentKeyDown);
    return () => document.removeEventListener("keydown", handleDocumentKeyDown);
  });

  function selectRelativeArticle(offset: number): void {
    if (articles.length === 0) {
      return;
    }

    const currentIndex = Math.max(
      0,
      articles.findIndex((article) => article.id === selectedArticle?.id),
    );
    const nextIndex = clamp(currentIndex + offset, 0, articles.length - 1);
    setSelectedArticleId(articles[nextIndex].id);
  }

  function handlePaneResizeStart(pane: PaneKey, event: PointerEvent<HTMLDivElement>): void {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = paneWidths[pane];

    function handlePointerMove(moveEvent: globalThis.PointerEvent): void {
      const nextWidth = startWidth + moveEvent.clientX - startX;
      setPaneWidths((current) => ({
        ...current,
        [pane]: clamp(nextWidth, paneBounds[pane].min, paneBounds[pane].max),
      }));
    }

    function handlePointerUp(): void {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    }

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
  }

  function handleResetWorkspaceLayout(): void {
    setPaneWidths(defaultPaneWidths);
    setEntryLayout("list");
    localStorage.removeItem(paneStorageKey);
    localStorage.removeItem(entryLayoutStorageKey);
  }

  async function runTask(
    label: string,
    task: () => Promise<void>,
    onError?: (message: string) => void,
  ): Promise<void> {
    setIsBusy(true);
    setStatus(label);
    try {
      await task();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setStatus(message);
      onError?.(message);
    } finally {
      setIsBusy(false);
    }
  }

  const shellStyle = {
    "--sidebar-width": `${paneWidths.sidebar}px`,
    "--timeline-width": `${paneWidths.timeline}px`,
  } as CSSProperties;

  return (
    <main
      className="app-shell"
      data-view={activeView}
      style={shellStyle}
    >
      <IconRail
        activeView={activeView}
        onSelectView={setActiveView}
        themeMode={themeMode}
        onCycleTheme={() => setThemeMode((mode) => nextThemeMode(mode))}
      />
      <datalist id={categoryDatalistId}>
        {categoryOptions.map((category) => (
          <option key={category} value={category} />
        ))}
      </datalist>
      <aside className="sidebar">
        <div className="sidebar-header">
          <div className="brand">
            <span className="brand-mark">F</span>
            <div>
              <strong>Feader</strong>
              <span>Local source desk</span>
            </div>
          </div>
          <span className="sync-pill">{isBusy ? "Syncing" : "Ready"}</span>
        </div>

        <div className="source-stats" aria-label="Library summary">
          <div>
            <strong>{sources.length}</strong>
            <span>Sources</span>
          </div>
          <div>
            <strong>{unreadCount}</strong>
            <span>Unread</span>
          </div>
          <div>
            <strong>{failedSourceCount}</strong>
            <span>Alerts</span>
          </div>
        </div>

        {activeView === "reader" ? (
          <>
            <button
              className="secondary-action full-width"
              disabled={isBusy}
              onClick={handleRefreshAll}
              type="button"
            >
              Refresh all sources
            </button>

            <nav className="feed-list" aria-label="Feeds">
              <button
                className={`feed-item ${selectedSourceId === undefined ? "active" : ""}`}
                onClick={() => void handleSelectSource(undefined)}
                type="button"
              >
                <span className="feed-main">
                  <span className="status-dot mixed" />
                  <span className="feed-name">All feeds</span>
                </span>
                <small>{unreadCount}</small>
              </button>
              {sourceGroups.map((group) => {
                const collapsed = collapsedGroups[group.category] ?? false;
                return (
                  <div className="feed-group" key={group.category}>
                    <button
                      aria-expanded={!collapsed}
                      className="feed-group-header"
                      onClick={() =>
                        setCollapsedGroups((current) => ({
                          ...current,
                          [group.category]: !collapsed,
                        }))
                      }
                      type="button"
                    >
                      <span>{group.category}</span>
                      <span aria-hidden="true">{collapsed ? "▸" : "▾"}</span>
                    </button>
                    {collapsed
                      ? null
                      : group.sources.map((source) => (
                          <button
                            className={`feed-item ${selectedSourceId === source.id ? "active" : ""}`}
                            key={source.id}
                            onClick={() => void handleSelectSource(source.id)}
                            type="button"
                          >
                            <span className="feed-main">
                              <span
                                className={`status-dot ${source.lastError ? "error" : source.unreadCount > 0 ? "healthy" : "muted"}`}
                              />
                              <span className="feed-name">{source.title}</span>
                            </span>
                            <small>{source.unreadCount}</small>
                          </button>
                        ))}
                  </div>
                );
              })}
            </nav>
          </>
        ) : null}
      </aside>

      {activeView === "reader" ? (
        <>
      <PaneResizer
        label="Resize source sidebar"
        onPointerDown={(event) => handlePaneResizeStart("sidebar", event)}
      />
      <section className="timeline" aria-label="Reading queue">
        <header className="topbar">
          <div>
            <p className="eyebrow">{selectedSource?.kind ?? "Library"} · {selectedSourceHealth}</p>
            <h1>{selectedSource?.title ?? "Reading queue"}</h1>
          </div>
          <div className="queue-metrics" aria-label="Queue summary">
            <span>{articles.length} shown</span>
            <span>{unreadCount} unread</span>
          </div>
          <div className="topbar-actions">
            <button
              className="secondary-action compact-action"
              disabled={isBusy || articles.length === 0}
              onClick={handleMarkAllRead}
              type="button"
            >
              Mark all read
            </button>
            <button
              className="primary-action"
              disabled={isBusy || !selectedSourceId}
              onClick={() => selectedSourceId && void handleRefreshSource(selectedSourceId)}
              type="button"
            >
              Refresh
            </button>
          </div>
        </header>

        <div className="timeline-toolbar">
          <div className="filter-tabs" role="tablist" aria-label="Article filters">
            {(["all", "unread", "saved"] as const).map((mode) => (
              <button
                aria-selected={filterMode === mode}
                className={filterMode === mode ? "active" : ""}
                key={mode}
                onClick={() => void handleSetFilter(mode)}
                role="tab"
                type="button"
              >
                {filterLabel(mode)}
              </button>
            ))}
          </div>
          <EntryLayoutControl layout={entryLayout} onChange={setEntryLayout} />
          <div className="status-line">{status}</div>
        </div>

        <div className={`story-list ${entryLayout}`}>
          {articles.length === 0 ? (
            <section className="empty-state">
              <h2>No articles</h2>
              <p>{emptyStateCopy(filterMode)}</p>
            </section>
          ) : (
            articles.map((article) => (
              <article
                className={`story-card ${article.read ? "read" : ""} ${
                  selectedArticle?.id === article.id ? "selected" : ""
                }`}
                key={article.id}
                onKeyDown={(event) => handleArticleKeyDown(event, article.id)}
                onClick={() => setSelectedArticleId(article.id)}
                onDoubleClick={() => {
                  setSelectedArticleId(article.id);
                  setReaderView("immersive");
                }}
                role="button"
                tabIndex={0}
              >
                {entryLayout === "card" ? (
                  <div
                    className="story-thumb"
                    style={
                      article.imageUrl ? { backgroundImage: `url(${article.imageUrl})` } : undefined
                    }
                  />
                ) : null}
                <div className="story-state">
                  <span className={article.read ? "read-dot" : "unread-dot"} />
                  {article.saved ? <span className="saved-chip">Saved</span> : null}
                </div>
                <div className="story-meta">
                  <span>{article.sourceTitle}</span>
                  <span>{formatDate(article.publishedAt ?? article.createdAt)}</span>
                </div>
                <h2>{article.title}</h2>
                {article.summary ? <p>{stripHtml(article.summary)}</p> : null}
                <div className="story-actions">
                  <button onClick={(event) => {
                    event.stopPropagation();
                    void handleToggleRead(article);
                  }} type="button">
                    {article.read ? "Unread" : "Read"}
                  </button>
                  <button onClick={(event) => {
                    event.stopPropagation();
                    void handleToggleSaved(article);
                  }} type="button">
                    {article.saved ? "Unsave" : "Save"}
                  </button>
                  <a href={article.url} onClick={(event) => event.stopPropagation()} rel="noreferrer" target="_blank">
                    Open
                  </a>
                </div>
              </article>
            ))
          )}
        </div>
      </section>

        </>
      ) : null}

      {activeView === "sources" ? (
        <section className="page-view" aria-label="Sources">
          <header className="page-header">
            <div>
              <p className="eyebrow">Sources</p>
              <h1>Source manager</h1>
            </div>
            <button
              className="primary-action add-source-action"
              onClick={() => setShowSourceComposer((value) => !value)}
              type="button"
            >
              {showSourceComposer ? "Close source form" : "Add source"}
            </button>
          </header>

          {showSourceComposer ? (
            <section className="source-composer page-panel" aria-label="Add source">
              <div className="panel-heading">
                <span>New source</span>
                <span>{sourceInputModeLabel(sourceInputMode)}</span>
              </div>
              <form className="feed-form" onSubmit={handleAddFeed}>
                <section className="adapter-workbench">
                  <div className="adapter-rail" role="tablist" aria-label="Source type">
                    {(["rss", "xpath"] as const).map((mode) => (
                      <button
                        aria-selected={sourceInputMode === mode}
                        className={sourceInputMode === mode ? "active" : ""}
                        key={mode}
                        onClick={() => setSourceInputMode(mode)}
                        role="tab"
                        type="button"
                      >
                        <strong>{sourceInputModeLabel(mode)}</strong>
                        <span>{sourceInputModeKind(mode)}</span>
                      </button>
                    ))}
                  </div>
                  <div className="adapter-panel">
                    <input
                      aria-label={sourceInputMode === "rss" ? "Feed URL" : "Page URL"}
                      disabled={isBusy}
                      onChange={(event) => setFeedUrl(event.currentTarget.value)}
                      placeholder={
                        sourceInputMode === "rss"
                          ? "https://example.com/feed.xml"
                          : "https://example.com/articles"
                      }
                      value={feedUrl}
                    />
                    {sourceInputMode === "xpath" ? (
                      <XPathSourceForm
                        aiAvailable={aiSettings.enabled && aiSettings.apiKeySet}
                        isBusy={isBusy}
                        onPreview={() => void handlePreviewXPath()}
                        onSelectorsChange={setXPathSelectors}
                        onSuggest={() => void handleSuggestXPath()}
                        onTitleChange={setXPathTitle}
                        preview={xpathPreview}
                        selectors={xpathSelectors}
                        status={xpathStatus}
                        title={xpathTitle}
                      />
                    ) : (
                      <div className="adapter-summary" aria-label="RSS adapter status">
                        <span>Native parser</span>
                        <span>RSS</span>
                        <span>Atom</span>
                      </div>
                    )}
                    <button className="primary-action" disabled={isBusy} type="submit">
                      {sourceInputMode === "rss" ? "Add source" : "Confirm source"}
                    </button>
                  </div>
                </section>
              </form>
            </section>
          ) : null}

          <div className="source-grid">
            {sources.length === 0 ? (
              <section className="empty-state">
                <h2>No sources</h2>
                <p>Add a source to start building the local reader.</p>
              </section>
            ) : (
              sources.map((source) => (
                <article className="source-card" key={source.id}>
                  <div className="panel-heading">
                    <span>{source.title}</span>
                    <span>{sourceHealth(source)}</span>
                  </div>
                  <SourceHealthStrip source={source} />
                  <CategoryPicker
                    disabled={isBusy}
                    onSubmit={(id, category) => void handleSetCategory(id, category)}
                    source={source}
                  />
                  <SourceCardManage
                    disabled={isBusy}
                    onDelete={(id, title) => void handleDeleteSourceId(id, title)}
                    onRename={(id, title) => void handleRenameSourceId(id, title)}
                    source={source}
                  />
                  <dl>
                    <dt>Kind</dt>
                    <dd>{source.kind}</dd>
                    <dt>URL</dt>
                    <dd>{source.url}</dd>
                    <dt>Enabled</dt>
                    <dd>{source.enabled ? "Yes" : "No"}</dd>
                    <dt>Unread</dt>
                    <dd>{source.unreadCount}</dd>
                    <dt>Articles</dt>
                    <dd>{source.articleCount}</dd>
                    <dt>Last refresh</dt>
                    <dd>{formatDate(source.lastFetchedAt)}</dd>
                    <dt>Status</dt>
                    <dd>{sourceDiagnostic(source)}</dd>
                  </dl>
                  {source.lastError ? <p className="error-text">{source.lastError}</p> : null}
                  <div className="story-actions">
                    <button
                      disabled={isBusy}
                      onClick={() => {
                        setActiveView("reader");
                        void handleSelectSource(source.id);
                      }}
                      type="button"
                    >
                      Select
                    </button>
                    <button
                      disabled={isBusy}
                      onClick={() => void handleRefreshSource(source.id)}
                      type="button"
                    >
                      Refresh
                    </button>
                  </div>
                </article>
              ))
            )}
          </div>
        </section>
      ) : null}

      {activeView === "settings" ? (
        <section className="page-view" aria-label="Settings">
          <header className="page-header">
            <div>
              <p className="eyebrow">Settings</p>
              <h1>Preferences</h1>
            </div>
          </header>

          <section className="settings-grid">
            <article className="settings-card">
              <div className="panel-heading">
                <span>Appearance</span>
                <span>{themeLabel(themeMode)}</span>
              </div>
              <ThemeControl mode={themeMode} onChange={setThemeMode} />
            </article>

            <WalletLoginCard
              chainId={chainId}
              isBusy={isBusy}
              isConnected={isConnected}
              onConnect={() => void handleConnectWallet()}
              onDisconnect={() => void handleWalletDisconnect()}
              onSignIn={() => void handleWalletSignIn()}
              session={walletSession}
              walletAddress={walletAddress}
            />

            <AiSettingsCard
              disabled={isBusy}
              onSave={(input) => void handleSaveAiSettings(input)}
              settings={aiSettings}
            />

            <article className="settings-card">
              <div className="panel-heading">
                <span>Workspace</span>
                <span>{entryLayoutLabel(entryLayout)}</span>
              </div>
              <EntryLayoutControl layout={entryLayout} onChange={setEntryLayout} />
              <dl>
                <dt>Sources</dt>
                <dd>{sources.length}</dd>
                <dt>Unread</dt>
                <dd>{unreadCount}</dd>
                <dt>Alerts</dt>
                <dd>{failedSourceCount}</dd>
                <dt>Sidebar</dt>
                <dd>{paneWidths.sidebar}px</dd>
                <dt>Timeline</dt>
                <dd>{paneWidths.timeline}px</dd>
              </dl>
              <button className="secondary-action" onClick={handleResetWorkspaceLayout} type="button">
                Reset workspace layout
              </button>
            </article>

            <article className="settings-card">
              <div className="panel-heading">
                <span>Reader</span>
                <span>{readerTypographyLabel(readerTypography)}</span>
              </div>
              <ReaderTypographyControl
                mode={readerTypography}
                onChange={setReaderTypography}
              />
              <dl>
                <dt>Body</dt>
                <dd>{readerTypographyLabel(readerTypography)}</dd>
                <dt>Actions</dt>
                <dd>Sticky</dd>
              </dl>
            </article>

            <article className="settings-card">
              <div className="panel-heading">
                <span>Reading flow</span>
                <span>{selectedArticle ? "Active" : "Idle"}</span>
              </div>
              <div className="preference-strip">
                <span>{articles.length} queued</span>
                <span>{selectedArticle ? "Article selected" : "No selection"}</span>
                <span>{filterLabel(filterMode)}</span>
              </div>
              <dl>
                <dt>Queue</dt>
                <dd>{articles.length}</dd>
                <dt>Selected</dt>
                <dd>{selectedArticle?.title ?? "None"}</dd>
                <dt>Filter</dt>
                <dd>{filterLabel(filterMode)}</dd>
              </dl>
            </article>
          </section>
        </section>
      ) : null}

      {readerView === "preview" && selectedArticle ? (
        <div
          className="ql-backdrop"
          onClick={() => setReaderView("none")}
          role="presentation"
        >
          <div
            aria-label={selectedArticle.title}
            aria-modal="true"
            className="ql-panel"
            onClick={(event) => event.stopPropagation()}
            role="dialog"
          >
            <button
              aria-label="Close preview"
              className="ql-close"
              onClick={() => setReaderView("none")}
              type="button"
            >
              x
            </button>
            <ReaderArticle
              article={selectedArticle}
              onToggleRead={(item) => void handleToggleRead(item)}
              onToggleSaved={(item) => void handleToggleSaved(item)}
              readerTypography={readerTypography}
            />
          </div>
        </div>
      ) : null}

      {readerView === "immersive" && selectedArticle ? (
        <div aria-label="Immersive reader" aria-modal="true" className="immersive" role="dialog">
          <div className="immersive-bar">
            <span>{selectedArticle.sourceTitle}</span>
            <button
              aria-label="Exit immersive reading"
              className="secondary-action"
              onClick={() => setReaderView("none")}
              type="button"
            >
              Close
            </button>
          </div>
          <div className="immersive-body">
            <ReaderArticle
              article={selectedArticle}
              onToggleRead={(item) => void handleToggleRead(item)}
              onToggleSaved={(item) => void handleToggleSaved(item)}
              readerTypography={readerTypography}
            />
          </div>
        </div>
      ) : null}
    </main>
  );
}

function IconRail({
  activeView,
  onSelectView,
  themeMode,
  onCycleTheme,
}: {
  activeView: ViewMode;
  onSelectView: (view: ViewMode) => void;
  themeMode: ThemeMode;
  onCycleTheme: () => void;
}) {
  return (
    <nav className="icon-rail" aria-label="Primary">
      <span className="rail-mark" aria-hidden="true">F</span>
      {(["reader", "sources"] as const).map((view) => (
        <button
          aria-current={activeView === view ? "page" : undefined}
          aria-label={viewLabel(view)}
          className={`rail-button ${activeView === view ? "active" : ""}`}
          key={view}
          onClick={() => onSelectView(view)}
          type="button"
        >
          {railIcon(view)}
        </button>
      ))}
      <span className="rail-spacer" />
      <button
        aria-label={`Theme: ${themeLabel(themeMode)}`}
        className="rail-button"
        onClick={onCycleTheme}
        type="button"
      >
        {railIcon("theme")}
      </button>
      <button
        aria-current={activeView === "settings" ? "page" : undefined}
        aria-label="Settings"
        className={`rail-button ${activeView === "settings" ? "active" : ""}`}
        onClick={() => onSelectView("settings")}
        type="button"
      >
        {railIcon("settings")}
      </button>
    </nav>
  );
}

function railIcon(name: ViewMode | "theme") {
  const paths: Record<string, string> = {
    reader: "M4 6h16M4 12h16M4 18h11",
    sources: "M4 4h16v16H4zM4 9.5h16",
    theme: "M12 7a5 5 0 100 10 5 5 0 000-10zM12 2v2M12 20v2M2 12h2M20 12h2",
    settings: "M12 9a3 3 0 100 6 3 3 0 000-6zM12 2v3M12 19v3M2 12h3M19 12h3",
  };
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <path d={paths[name]} />
    </svg>
  );
}

function nextThemeMode(mode: ThemeMode): ThemeMode {
  if (mode === "light") {
    return "dark";
  }
  if (mode === "dark") {
    return "system";
  }
  return "light";
}

function ThemeControl({
  mode,
  onChange,
}: {
  mode: ThemeMode;
  onChange: (mode: ThemeMode) => void;
}) {
  return (
    <div className="theme-control" role="group" aria-label="Theme">
      {(["light", "dark", "system"] as const).map((theme) => (
        <button
          className={mode === theme ? "active" : ""}
          key={theme}
          onClick={() => onChange(theme)}
          type="button"
        >
          {themeLabel(theme)}
        </button>
      ))}
    </div>
  );
}

function AiSettingsCard({
  settings,
  disabled,
  onSave,
}: {
  settings: AiSettings;
  disabled: boolean;
  onSave: (input: {
    provider: AiProvider;
    baseUrl: string;
    model: string;
    enabled: boolean;
    apiKey?: string;
  }) => void;
}) {
  const [provider, setProvider] = useState<AiProvider>(settings.provider);
  const [baseUrl, setBaseUrl] = useState(settings.baseUrl);
  const [model, setModel] = useState(settings.model);
  const [enabled, setEnabled] = useState(settings.enabled);
  const [apiKey, setApiKey] = useState("");

  useEffect(() => {
    setProvider(settings.provider);
    setBaseUrl(settings.baseUrl);
    setModel(settings.model);
    setEnabled(settings.enabled);
    setApiKey("");
  }, [settings]);

  return (
    <article className="settings-card">
      <div className="panel-heading">
        <span>AI</span>
        <span>{settings.enabled && settings.apiKeySet ? "Active" : "Off"}</span>
      </div>
      <form
        className="ai-form"
        onSubmit={(event) => {
          event.preventDefault();
          onSave({ provider, baseUrl, model, enabled, apiKey: apiKey.trim() || undefined });
        }}
      >
        <label className="selector-input">
          <span>Provider</span>
          <select
            disabled={disabled}
            onChange={(event) => setProvider(event.currentTarget.value as AiProvider)}
            value={provider}
          >
            <option value="openai">OpenAI-compatible</option>
            <option value="anthropic">Anthropic</option>
          </select>
        </label>
        <label className="selector-input">
          <span>Base URL</span>
          <input
            disabled={disabled}
            onChange={(event) => setBaseUrl(event.currentTarget.value)}
            placeholder={
              provider === "anthropic" ? "https://api.anthropic.com" : "https://api.openai.com/v1"
            }
            value={baseUrl}
          />
        </label>
        <label className="selector-input">
          <span>Model</span>
          <input
            disabled={disabled}
            onChange={(event) => setModel(event.currentTarget.value)}
            placeholder={provider === "anthropic" ? "claude-haiku-4-5" : "gpt-4o-mini"}
            value={model}
          />
        </label>
        <label className="selector-input">
          <span>API key {settings.apiKeySet ? "(set, blank keeps it)" : ""}</span>
          <input
            disabled={disabled}
            onChange={(event) => setApiKey(event.currentTarget.value)}
            placeholder="sk-... or $MY_API_KEY"
            type="password"
            value={apiKey}
          />
          {settings.apiKeyReference ? (
            <small className="selector-hint">
              Using environment reference {settings.apiKeyReference}
            </small>
          ) : settings.apiKeySet ? (
            <small className="selector-hint">
              Literal key is stored locally; leave blank to keep it.
            </small>
          ) : null}
        </label>
        <label className="ai-enable">
          <input
            checked={enabled}
            disabled={disabled}
            onChange={(event) => setEnabled(event.currentTarget.checked)}
            type="checkbox"
          />
          <span>Enable AI features</span>
        </label>
        <div className="ai-actions">
          <button className="primary-action" disabled={disabled} type="submit">
            Save AI settings
          </button>
          <a href={aiDocsUrl} rel="noreferrer" target="_blank">
            Configuration guide
          </a>
        </div>
      </form>
    </article>
  );
}

function EntryLayoutControl({
  layout,
  onChange,
}: {
  layout: EntryLayout;
  onChange: (layout: EntryLayout) => void;
}) {
  return (
    <div className="entry-layout-control" role="group" aria-label="Entry layout">
      {(["list", "card"] as const).map((next) => (
        <button
          aria-pressed={layout === next}
          className={layout === next ? "active" : ""}
          key={next}
          onClick={() => onChange(next)}
          type="button"
        >
          {entryLayoutLabel(next)}
        </button>
      ))}
    </div>
  );
}

function CategoryPicker({
  source,
  disabled,
  onSubmit,
}: {
  source: Source;
  disabled: boolean;
  onSubmit: (sourceId: number, category: string) => void;
}) {
  const [value, setValue] = useState(source.category ?? "");
  useEffect(() => {
    setValue(source.category ?? "");
  }, [source.id, source.category]);

  return (
    <form
      className="category-picker"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit(source.id, value);
      }}
    >
      <input
        aria-label="Source category"
        disabled={disabled}
        list={categoryDatalistId}
        onChange={(event) => setValue(event.currentTarget.value)}
        placeholder="Category"
        value={value}
      />
      <button disabled={disabled} type="submit">
        Set
      </button>
    </form>
  );
}

function SourceCardManage({
  source,
  disabled,
  onRename,
  onDelete,
}: {
  source: Source;
  disabled: boolean;
  onRename: (sourceId: number, title: string) => void;
  onDelete: (sourceId: number, title: string) => void;
}) {
  const [title, setTitle] = useState("");
  return (
    <>
      <form
        className="rename-form"
        onSubmit={(event) => {
          event.preventDefault();
          onRename(source.id, title || source.title);
          setTitle("");
        }}
      >
        <input
          aria-label={`Rename ${source.title}`}
          disabled={disabled}
          onChange={(event) => setTitle(event.currentTarget.value)}
          placeholder={source.title}
          value={title}
        />
        <button disabled={disabled} type="submit">
          Rename
        </button>
      </form>
      <button
        className="danger-action"
        disabled={disabled}
        onClick={() => onDelete(source.id, source.title)}
        type="button"
      >
        Delete feed
      </button>
    </>
  );
}

function ReaderTypographyControl({
  mode,
  onChange,
}: {
  mode: ReaderTypography;
  onChange: (mode: ReaderTypography) => void;
}) {
  return (
    <div className="reader-type-control" role="group" aria-label="Reader typography">
      {(["system", "serif", "large"] as const).map((nextMode) => (
        <button
          className={mode === nextMode ? "active" : ""}
          key={nextMode}
          onClick={() => onChange(nextMode)}
          type="button"
        >
          {readerTypographyLabel(nextMode)}
        </button>
      ))}
    </div>
  );
}

function ReaderArticle({
  article,
  readerTypography,
  onToggleRead,
  onToggleSaved,
}: {
  article: Article;
  readerTypography: ReaderTypography;
  onToggleRead: (article: Article) => void;
  onToggleSaved: (article: Article) => void;
}) {
  const sanitizedHtml = useMemo(
    () => (article.contentHtml ? sanitizeArticleHtml(article.contentHtml) : ""),
    [article.contentHtml],
  );

  return (
    <article className="reader-article" data-typography={readerTypography}>
      <div className="reader-kicker">
        <span>{article.sourceTitle}</span>
        <span>{formatDate(article.publishedAt ?? article.createdAt)}</span>
      </div>
      <h2>{article.title}</h2>
      {article.author ? <p className="byline">{article.author}</p> : null}
      <div className="reader-actions">
        <button onClick={() => onToggleRead(article)} type="button">
          {article.read ? "Mark unread" : "Mark read"}
        </button>
        <button onClick={() => onToggleSaved(article)} type="button">
          {article.saved ? "Unsave" : "Save"}
        </button>
        <a href={article.url} rel="noreferrer" target="_blank">
          Open full page
        </a>
      </div>
      <dl className="reader-meta">
        <dt>Source</dt>
        <dd>{article.sourceTitle}</dd>
        <dt>Published</dt>
        <dd>{formatDate(article.publishedAt ?? article.createdAt)}</dd>
        <dt>Body</dt>
        <dd>{articleBodyState(article)}</dd>
        {article.canonicalUrl ? (
          <>
            <dt>Canonical</dt>
            <dd>{article.canonicalUrl}</dd>
          </>
        ) : null}
      </dl>
      {article.imageUrl ? (
        <img alt="" className="reader-image" src={article.imageUrl} />
      ) : null}
      <div className="reader-body">
        {sanitizedHtml ? (
          <div dangerouslySetInnerHTML={{ __html: sanitizedHtml }} />
        ) : article.contentText ? (
          <p>{article.contentText}</p>
        ) : article.summary ? (
          <p>{stripHtml(article.summary)}</p>
        ) : (
          <p>{articleBodyFallback(article)}</p>
        )}
      </div>
    </article>
  );
}

function PaneResizer({
  label,
  onPointerDown,
}: {
  label: string;
  onPointerDown: (event: PointerEvent<HTMLDivElement>) => void;
}) {
  return (
    <div
      aria-label={label}
      className="pane-resizer"
      onPointerDown={onPointerDown}
      role="separator"
      tabIndex={-1}
    />
  );
}

function SourceHealthStrip({ source }: { source: Source }) {
  return (
    <div className="source-health-strip" aria-label={`${source.title} source health`}>
      <span className={`health-chip ${sourceHealthClass(source)}`}>{sourceHealth(source)}</span>
      <span>{source.kind.toUpperCase()}</span>
      <span>{source.enabled ? "Enabled" : "Disabled"}</span>
      <span>{formatDate(source.lastFetchedAt)}</span>
    </div>
  );
}

function themeLabel(mode: ThemeMode): string {
  if (mode === "light") {
    return "Light";
  }
  if (mode === "dark") {
    return "Dark";
  }
  return "System";
}

function viewLabel(mode: ViewMode): string {
  if (mode === "reader") {
    return "Reader";
  }
  if (mode === "sources") {
    return "Sources";
  }
  return "Settings";
}

function sourceInputModeLabel(mode: SourceInputMode): string {
  if (mode === "xpath") {
    return "XPath";
  }
  return "RSS/Atom";
}

function sourceInputModeKind(mode: SourceInputMode): string {
  if (mode === "xpath") {
    return "Declarative";
  }
  return "Native";
}

function entryLayoutLabel(layout: EntryLayout): string {
  return layout === "card" ? "Card" : "List";
}

function readerTypographyLabel(mode: ReaderTypography): string {
  if (mode === "serif") {
    return "Serif";
  }
  if (mode === "large") {
    return "Large";
  }
  return "System";
}

function readInitialThemeMode(): ThemeMode {
  const stored = localStorage.getItem(themeStorageKey);
  if (stored === "light" || stored === "dark" || stored === "system") {
    return stored;
  }
  return "system";
}

function readInitialEntryLayout(): EntryLayout {
  const stored = localStorage.getItem(entryLayoutStorageKey);
  if (stored === "list" || stored === "card") {
    return stored;
  }
  return "list";
}

function readInitialReaderTypography(): ReaderTypography {
  const stored = localStorage.getItem(readerTypographyStorageKey);
  if (stored === "system" || stored === "serif" || stored === "large") {
    return stored;
  }
  return "system";
}

function readInitialPaneWidths(): PaneWidths {
  const stored = localStorage.getItem(paneStorageKey);
  if (!stored) {
    return defaultPaneWidths;
  }

  try {
    const parsed = JSON.parse(stored) as Partial<PaneWidths>;
    return {
      sidebar: clamp(
        parsed.sidebar ?? defaultPaneWidths.sidebar,
        paneBounds.sidebar.min,
        paneBounds.sidebar.max,
      ),
      timeline: clamp(
        parsed.timeline ?? defaultPaneWidths.timeline,
        paneBounds.timeline.min,
        paneBounds.timeline.max,
      ),
    };
  } catch {
    return defaultPaneWidths;
  }
}

function applyThemeMode(mode: ThemeMode): void {
  const resolved =
    mode === "system"
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : mode;
  document.documentElement.dataset.theme = resolved;
  document.documentElement.dataset.themePreference = mode;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function isTextInputTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return (
    target.tagName === "INPUT" ||
    target.tagName === "TEXTAREA" ||
    target.tagName === "SELECT" ||
    target.isContentEditable
  );
}

function buildArticleFilter(sourceId: number | undefined, mode: FilterMode): ArticleFilter | null {
  const filter: ArticleFilter = {};
  if (sourceId) {
    filter.sourceId = sourceId;
  }
  if (mode === "unread") {
    filter.unreadOnly = true;
  }
  if (mode === "saved") {
    filter.savedOnly = true;
  }
  return Object.keys(filter).length === 0 ? null : filter;
}

function XPathSourceForm({
  aiAvailable,
  isBusy,
  onPreview,
  onSelectorsChange,
  onSuggest,
  onTitleChange,
  preview,
  selectors,
  status,
  title,
}: {
  aiAvailable: boolean;
  isBusy: boolean;
  onPreview: () => void;
  onSelectorsChange: (selectors: XPathSelectors) => void;
  onSuggest: () => void;
  onTitleChange: (title: string) => void;
  preview: XPathPreview | null;
  selectors: XPathSelectors;
  status: string | null;
  title: string;
}) {
  const previewArticles = preview?.articles ?? [];
  return (
    <section className="xpath-form">
      <label className="selector-input">
        <span>Preset</span>
        <select
          aria-label="Selector preset"
          disabled={isBusy}
          onChange={(event) => {
            const preset = xpathPresets[event.currentTarget.value];
            if (preset) {
              onSelectorsChange(preset);
            }
          }}
          value=""
        >
          <option value="">Choose a preset…</option>
          {Object.keys(xpathPresets).map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
      </label>
      <input
        aria-label="XPath source title"
        disabled={isBusy}
        onChange={(event) => onTitleChange(event.currentTarget.value)}
        placeholder="Source title"
        value={title}
      />
      <SelectorInput
        disabled={isBusy}
        hint="Repeating element per article, e.g. //article"
        label="Items"
        name="items"
        onChange={onSelectorsChange}
        selectors={selectors}
      />
      <SelectorInput
        disabled={isBusy}
        hint="Text or link inside an item, e.g. .//h2/a"
        label="Title"
        name="title"
        onChange={onSelectorsChange}
        selectors={selectors}
      />
      <SelectorInput
        disabled={isBusy}
        hint="Link href inside an item, e.g. .//h2/a/@href"
        label="URL"
        name="url"
        onChange={onSelectorsChange}
        selectors={selectors}
      />
      <SelectorInput
        disabled={isBusy}
        label="Summary"
        name="summary"
        onChange={onSelectorsChange}
        selectors={selectors}
      />
      <SelectorInput
        disabled={isBusy}
        label="Date"
        name="publishedAt"
        onChange={onSelectorsChange}
        selectors={selectors}
      />
      <details className="xpath-advanced">
        <summary>Advanced fields</summary>
        <SelectorInput
          disabled={isBusy}
          label="Author"
          name="author"
          onChange={onSelectorsChange}
          selectors={selectors}
        />
        <SelectorInput
          disabled={isBusy}
          label="Content"
          name="content"
          onChange={onSelectorsChange}
          selectors={selectors}
        />
        <SelectorInput
          disabled={isBusy}
          label="Image"
          name="image"
          onChange={onSelectorsChange}
          selectors={selectors}
        />
        <SelectorInput
          disabled={isBusy}
          label="Next page"
          name="nextPage"
          onChange={onSelectorsChange}
          selectors={selectors}
        />
      </details>
      {aiAvailable ? (
        <button disabled={isBusy} onClick={onSuggest} type="button">
          Suggest with AI
        </button>
      ) : null}
      <button disabled={isBusy} onClick={onPreview} type="button">
        Preview
      </button>
      {status ? <p className="xpath-status">{status}</p> : null}
      {preview ? (
        <div className="xpath-diagnostics" aria-label="XPath selector diagnostics">
          {preview.diagnostics.map((diagnostic) => (
            <div
              className="xpath-diagnostic"
              data-status={diagnostic.status}
              key={diagnostic.field}
            >
              <span>{diagnostic.label}</span>
              <strong>{diagnostic.status}</strong>
              <em>{diagnostic.sample || diagnostic.message}</em>
            </div>
          ))}
          {preview.nextPageUrl ? (
            <div className="xpath-diagnostic" data-status="ok">
              <span>Next page</span>
              <strong>ok</strong>
              <em>{preview.nextPageUrl}</em>
            </div>
          ) : null}
        </div>
      ) : null}
      {previewArticles.length > 0 ? (
        <div className="xpath-preview">
          {previewArticles.map((article) => (
            <article key={article.url}>
              <strong>{article.title}</strong>
              <span>{article.url}</span>
              {article.summary ? <p>{article.summary}</p> : null}
              {article.author ? <em>{article.author}</em> : null}
            </article>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function SelectorInput({
  disabled,
  hint,
  label,
  name,
  onChange,
  selectors,
}: {
  disabled: boolean;
  hint?: string;
  label: string;
  name: keyof XPathSelectors;
  onChange: (selectors: XPathSelectors) => void;
  selectors: XPathSelectors;
}) {
  return (
    <label className="selector-input">
      <span>{label}</span>
      <input
        disabled={disabled}
        onChange={(event) =>
          onChange({
            ...selectors,
            [name]: event.currentTarget.value,
          })
        }
        value={selectors[name] ?? ""}
      />
      {hint ? <small className="selector-hint">{hint}</small> : null}
    </label>
  );
}

function compactXPathSelectors(selectors: XPathSelectors): XPathSelectors {
  return {
    items: selectors.items.trim(),
    title: selectors.title.trim(),
    url: selectors.url.trim(),
    summary: emptyToUndefined(selectors.summary),
    publishedAt: emptyToUndefined(selectors.publishedAt),
    author: emptyToUndefined(selectors.author),
    content: emptyToUndefined(selectors.content),
    image: emptyToUndefined(selectors.image),
    nextPage: emptyToUndefined(selectors.nextPage),
  };
}

function normalizeXPathSelectorsForForm(selectors: XPathSelectors): XPathSelectors {
  return {
    items: selectors.items?.trim() || defaultXPathSelectors.items,
    title: selectors.title?.trim() || defaultXPathSelectors.title,
    url: selectors.url?.trim() || defaultXPathSelectors.url,
    summary: selectors.summary?.trim() || "",
    publishedAt: selectors.publishedAt?.trim() || "",
    author: selectors.author?.trim() || "",
    content: selectors.content?.trim() || "",
    image: selectors.image?.trim() || "",
    nextPage: selectors.nextPage?.trim() || "",
  };
}

function emptyToUndefined(value?: string): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function resolveSelectedArticleId(
  articles: Article[],
  preferredId: number | undefined,
): number | undefined {
  if (preferredId && articles.some((article) => article.id === preferredId)) {
    return preferredId;
  }
  return articles[0]?.id;
}

function WalletLoginCard({
  chainId,
  isBusy,
  isConnected,
  onConnect,
  onDisconnect,
  onSignIn,
  session,
  walletAddress,
}: {
  chainId?: number;
  isBusy: boolean;
  isConnected: boolean;
  onConnect: () => void;
  onDisconnect: () => void;
  onSignIn: () => void;
  session: WalletSession | null;
  walletAddress?: string;
}) {
  const verifiedMatches =
    Boolean(session && walletAddress) &&
    session?.address.toLowerCase() === walletAddress?.toLowerCase();

  return (
    <article className="settings-card wallet-card">
      <div className="panel-heading">
        <span>Account</span>
        <span>{session ? "Verified" : isConnected ? "Connected" : "Local"}</span>
      </div>
      <div className="wallet-status">
        <span>{isWalletConnectConfigured ? "WalletConnect" : "Injected wallet"}</span>
        <strong>{session ? shortAddress(session.address) : "Not signed in"}</strong>
        <em>{session ? `Chain ${session.chainId}` : "SIWE local session"}</em>
      </div>
      <dl>
        <dt>Wallet</dt>
        <dd>{walletAddress ? shortAddress(walletAddress) : "Disconnected"}</dd>
        <dt>Network</dt>
        <dd>{chainId ? `Chain ${chainId}` : "Unknown"}</dd>
        <dt>Session</dt>
        <dd>{session ? formatDate(session.signedInAt) : "None"}</dd>
      </dl>
      <div className="story-actions">
        <button disabled={isBusy || isConnected} onClick={onConnect} type="button">
          Connect wallet
        </button>
        <button
          className="primary-action"
          disabled={isBusy || !isConnected || verifiedMatches}
          onClick={onSignIn}
          type="button"
        >
          Sign in
        </button>
        <button disabled={isBusy || (!isConnected && !session)} onClick={onDisconnect} type="button">
          Disconnect
        </button>
      </div>
      {!isWalletConnectConfigured ? (
        <p className="wallet-note">
          Set VITE_REOWN_PROJECT_ID to enable WalletConnect QR login in the desktop app.
        </p>
      ) : null}
    </article>
  );
}

function buildSiweMessage(
  challenge: WalletLoginChallenge,
  address: string,
  chainId: number,
): string {
  return `${challenge.domain} wants you to sign in with your Ethereum account:
${address}

${challenge.statement}

URI: ${challenge.uri}
Version: 1
Chain ID: ${chainId}
Nonce: ${challenge.nonce}
Issued At: ${challenge.issuedAt}
Expiration Time: ${challenge.expiresAt}`;
}

function shortAddress(address: string): string {
  return address.length > 12 ? `${address.slice(0, 6)}...${address.slice(-4)}` : address;
}

function filterLabel(mode: FilterMode): string {
  if (mode === "unread") {
    return "Unread";
  }
  if (mode === "saved") {
    return "Saved";
  }
  return "All";
}

function emptyStateCopy(mode: FilterMode): string {
  if (mode === "unread") {
    return "No unread articles match this view.";
  }
  if (mode === "saved") {
    return "No saved articles match this view.";
  }
  return "Add or refresh a feed to populate the local reader.";
}

function sourceHealth(source: Source): string {
  if (source.lastError) {
    return "Attention";
  }
  if (source.lastFetchedAt) {
    return "Healthy";
  }
  return "New";
}

function sourceHealthClass(source: Source): string {
  if (source.lastError) {
    return "attention";
  }
  if (source.lastFetchedAt) {
    return "healthy";
  }
  return "new";
}

function sourceDiagnostic(source: Source): string {
  if (source.lastError) {
    return source.lastError;
  }
  if (source.lastFetchedAt) {
    return `Last refreshed ${formatDate(source.lastFetchedAt)}`;
  }
  return "Waiting for first refresh";
}

function articleBodyState(article: Article): string {
  if (article.contentText) {
    return "Text";
  }
  if (article.contentHtml) {
    return "HTML";
  }
  if (article.summary) {
    return "Summary";
  }
  return "Unavailable";
}

function articleBodyFallback(article: Article): string {
  return `${article.sourceTitle} did not provide a local article body for this entry.`;
}

function formatDate(value?: string | null): string {
  if (!value) {
    return "Never";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function stripHtml(value: string): string {
  return value.replace(/<[^>]*>/g, "").trim();
}

DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noreferrer");
  }
});

function sanitizeArticleHtml(value: string): string {
  return DOMPurify.sanitize(value, { USE_PROFILES: { html: true } });
}

export default App;
