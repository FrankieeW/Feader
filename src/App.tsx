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
  refreshIntervalSeconds?: number | null;
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
type SourceInputMode = "rss" | "rsshub" | "xpath";
type ThemeMode = "light" | "dark" | "system";
type ViewMode = "reader" | "sources" | "hub" | "settings";
type EntryLayout = "list" | "card";
type ReaderTypography = "system" | "serif" | "large";
type ReaderView = "none" | "preview" | "immersive";
type AppUiPluginId = string;
type AppUiThemeByMode = { light: AppUiPluginId | null; dark: AppUiPluginId | null };
type SourceListPluginId = "image-board" | "social-stream" | "dense-radar";
type HubInstallFilter = "all" | "installed" | "not-installed" | "needs-update";
type DetailViewPluginId = "magazine" | "focus" | "research" | "cinema";
type PaneKey = "sidebar" | "timeline";
type ReaderVideo =
  | {
      kind: "file";
      url: string;
      label: string;
      mimeType?: string;
      poster?: string | null;
    }
  | {
      kind: "embed";
      url: string;
      label: string;
    };

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
  cookie?: string;
  content?: string;
  detailContent?: string;
  contentCleanup?: ContentCleanupRule[];
  image?: string;
  nextPage?: string;
  customFields?: XPathCustomField[];
  maxItems?: number;
  plugin?: XPathSourcePluginInfo;
  reader?: ReaderConfig | null;
};

type ContentCleanupRule = {
  pattern: string;
  replacement?: string;
};

type XPathCustomField = {
  key: string;
  label?: string;
  xpath: string;
  scope?: "item" | "detail";
};

type ParsedArticleCustomField = {
  key: string;
  label: string;
  value: string;
};

type XPathSourceSuggestion = {
  title?: string | null;
  selectors: XPathSelectors;
};

type XPathSourcePluginInfo = {
  id: string;
  name: string;
  version: string;
  registry: string;
  trust: string;
  candidateId: string;
  pageType: string;
  capabilities: string[];
  authors?: PluginAuthor[];
};

type XPathRuleCandidate = {
  id: string;
  pageType: string;
  priority: number;
  detect: string[];
  promptRule?: string;
  selectors: XPathSelectors;
};

type PluginSection = {
  id: string;
  path: string[];
  url: string;
};

type PluginDefaults = {
  maxItems?: number;
  maxPages?: number;
};

type ParamOption = {
  value: string;
  label: string;
};

type PluginParam = {
  key: string;
  label: string;
  type: string;
  placeholder?: string;
  options?: ParamOption[];
  required: boolean;
};

type PluginParameters = {
  urlTemplate?: string;
  sections?: PluginSection[];
  params?: PluginParam[];
  defaults?: PluginDefaults;
};

type ReaderLayout = {
  typography?: "system" | "serif" | "large";
  width?: "narrow" | "normal" | "wide";
  immersive?: boolean;
};

type ReaderConfig = {
  removeSelectors?: string[];
  resolveRelativeUrls?: boolean;
  rewriteLinks?: boolean;
  showCustomFields?: boolean;
  layout?: ReaderLayout | null;
  css?: string | null;
};

type PluginAuth = {
  checkUrl: string;
  loggedInXpath: string;
};

type PluginCredential = {
  pluginId: string;
  cookieSet: boolean;
  cookieReference?: string | null;
  updatedAt?: string | null;
  lastCheckedAt?: string | null;
  lastCheckOk?: boolean | null;
  lastCheckMessage?: string | null;
};

type CredentialCheck = { ok: boolean; message: string; checkedAt: string };

type RssHubInstance = {
  id: string;
  name: string;
  baseUrl: string;
  maintainer: string;
  location?: string | null;
  official: boolean;
  builtin: boolean;
};

type RssHubSettings = {
  globalInstanceId: string;
  instances: RssHubInstance[];
};

type RssHubSourceConfig = {
  route: string;
  instanceId?: string | null;
};

type RssHubInstanceCheck = {
  ok: boolean;
  message: string;
  checkedUrl: string;
};

type AutoRefreshConfig = {
  enabled: boolean;
  globalIntervalSeconds: number;
  pluginOverrides: PluginRefreshOverride[];
  nextRefreshAt?: string | null;
};

type PluginRefreshOverride = {
  pluginId: string;
  pluginName: string;
  refreshIntervalSeconds: number;
};

type RefreshTickEvent = {
  refreshing: boolean;
  currentSourceId?: number | null;
  currentSourceTitle?: string | null;
  nextRefreshAt?: string | null;
  sourcesChecked: number;
  sourcesRefreshed: number;
};

type XPathRulePack = {
  id: string;
  name: string;
  version: string;
  apiVersion: string;
  kind: string;
  registry: string;
  trust: string;
  description: string;
  logo?: string | null;
  capabilities: string[];
  candidates: XPathRuleCandidate[];
  authors?: PluginAuthor[];
  parameters?: PluginParameters | null;
  auth?: PluginAuth | null;
  tokens?: Record<string, string> | null;
};

type PluginPermissions = {
  network?: string[];
  credentials?: string[];
  ui?: string[];
  storage?: string[];
  importExport?: string[];
  runtime?: string[];
  execution?: string | null;
};

type PluginSettingsPage = {
  title: string;
  sections: {
    id: string;
    title: string;
    description?: string | null;
    fields: {
      key: string;
      label: string;
      type: string;
      placeholder?: string | null;
      help?: string | null;
      default?: unknown;
      options?: ParamOption[];
    }[];
  }[];
};

type RuntimeSourcePlugin = {
  schemaVersion: string;
  id: string;
  name: string;
  version: string;
  runtime: {
    engine: string;
    package?: string | null;
    version?: string | null;
    entry?: string | null;
  };
  capabilities: string[];
  routeTemplates?: {
    id: string;
    label: string;
    routeTemplate: string;
    requiredCredentials?: string[];
  }[];
  settingsPage?: PluginSettingsPage | null;
};

type PluginPack = {
  id: string;
  name: string;
  version: string;
  apiVersion: string;
  kind: string;
  registry: string;
  trust: string;
  description: string;
  logo?: string | null;
  capabilities: string[];
  authors?: PluginAuthor[];
  permissions?: PluginPermissions | null;
  xpath?: XPathRulePack | null;
  view?: {
    schemaVersion: string;
    id: string;
    name: string;
    version: string;
    slot: string;
    description?: string | null;
    capabilities: string[];
    tokens?: Record<string, string> | null;
  } | null;
  runtime?: RuntimeSourcePlugin | null;
};

type MarketplacePluginPack = PluginPack & {
  installed: boolean;
  installedVersion?: string | null;
  sourceMarketId?: string | null;
  sourceMarketName?: string | null;
  sourceMarketRepository?: string | null;
};

type PluginMarket = {
  id: string;
  name: string;
  repository: string;
  rawBaseUrl: string;
  branch: string;
  builtin: boolean;
};

type PluginMarketTemplate = {
  path: string;
  files: string[];
};

type PluginAuthor = {
  name: string;
  evmAddress?: string | null;
  avatarUrl?: string | null;
  website?: string | null;
  email?: string | null;
  githubId?: string | null;
};

type ParsedArticle = {
  title: string;
  url: string;
  summary?: string | null;
  publishedAt?: string | null;
  author?: string | null;
  contentText?: string | null;
  imageUrl?: string | null;
  tagsJson?: string | null;
};

type ArticleCustomFieldValue = {
  label?: string;
  value: string;
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

type ViewPluginDefinition<T extends string> = {
  id: T;
  name: string;
  description: string;
  capability: string;
  tokens?: Record<string, string> | null;
};

const defaultXPathSelectors: XPathSelectors = {
  items: "//article",
  title: ".//h2/a/text()",
  url: ".//h2/a/@href",
  summary: ".//p/text()",
  publishedAt: ".//time/@datetime",
  author: "",
  cookie: "",
  content: ".",
  detailContent: "",
  contentCleanup: [],
  image: ".//img/@src",
  nextPage: "",
  customFields: [],
  maxItems: undefined,
  plugin: undefined,
};

const xpathPresets: Record<string, XPathSelectors> = {
  "Generic blog": {
    items: "//article",
    title: ".//h2/a | .//h3/a",
    url: ".//h2/a/@href | .//h3/a/@href",
    summary: ".//p",
    publishedAt: ".//time/@datetime",
    author: "",
    cookie: "",
    content: ".//section",
    detailContent: "",
    contentCleanup: [],
    image: ".//img/@src",
    nextPage: "//a[@rel='next']/@href",
    customFields: [],
    maxItems: undefined,
    plugin: undefined,
  },
  "Listing + links": {
    items: "//li[.//a]",
    title: ".//a",
    url: ".//a/@href",
    summary: "",
    publishedAt: "",
    author: "",
    cookie: "",
    content: "",
    detailContent: "",
    contentCleanup: [],
    image: ".//img/@src",
    nextPage: "",
    customFields: [],
    maxItems: undefined,
    plugin: undefined,
  },
};

const themeStorageKey = "feader.theme";
const entryLayoutStorageKey = "feader.entryLayout";
const appUiThemeByModeStorageKey = "feader.theme.appUiByMode";
const detailViewPluginStorageKey = "feader.plugin.detailView";
const installedViewPluginsStorageKey = "feader.plugin.installedViews";
const installedViewPluginVersionsStorageKey = "feader.plugin.installedViewVersions";
const sourceListViewBySourceStorageKey = "feader.sourceListView.bySource";
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

const sourceListPlugins: ViewPluginDefinition<SourceListPluginId>[] = [
  {
    id: "image-board",
    name: "Image Board",
    description: "Turns the source article queue into thumbnail-led cards for visual feeds.",
    capability: "sourceList.view",
  },
  {
    id: "social-stream",
    name: "Social Stream",
    description: "Adds avatar-like source marks and post-style spacing for social media feeds.",
    capability: "sourceList.view",
  },
  {
    id: "dense-radar",
    name: "Dense Radar",
    description: "Compacts the queue for fast scanning of many short updates.",
    capability: "sourceList.view",
  },
];

const detailViewPlugins: ViewPluginDefinition<DetailViewPluginId>[] = [
  {
    id: "magazine",
    name: "Magazine",
    description: "Uses larger art, wider titles, and editorial spacing for feature-style reading.",
    capability: "detail.view",
  },
  {
    id: "focus",
    name: "Focus",
    description: "Dims metadata and narrows the body for distraction-light long-form reading.",
    capability: "detail.view",
  },
  {
    id: "research",
    name: "Research",
    description: "Keeps metadata prominent and body blocks structured for reference-heavy articles.",
    capability: "detail.view",
  },
  {
    id: "cinema",
    name: "Cinema Detail View",
    description: "Official FeaderHub media-forward detail template for visual articles and video.",
    capability: "detail.view",
  },
];

const defaultAiSettings: AiSettings = {
  provider: "openai",
  baseUrl: "",
  model: "",
  enabled: false,
  apiKeySet: false,
  apiKeyReference: null,
  updatedAt: "",
};

const defaultRssHubSettings: RssHubSettings = {
  globalInstanceId: "rsshub-rssforever",
  instances: [
    {
      id: "rsshub-app",
      name: "RSSHub Official",
      baseUrl: "https://rsshub.app",
      maintainer: "DIYgod",
      location: "US",
      official: true,
      builtin: true,
    },
    {
      id: "rsshub-rssforever",
      name: "RSSForever",
      baseUrl: "https://rsshub.rssforever.com",
      maintainer: "Stille",
      location: "AE",
      official: false,
      builtin: true,
    },
  ],
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
  {
    id: 2,
    kind: "xpath",
    title: "XPath Demo",
    url: "https://example.com/articles",
    category: "Advanced",
    configJson: JSON.stringify(defaultXPathSelectors),
    enabled: true,
    createdAt: "2026-05-23T09:00:00.000Z",
    lastFetchedAt: "2026-05-23T09:00:00.000Z",
    lastError: null,
    articleCount: 0,
    unreadCount: 0,
  },
  {
    id: 3,
    kind: "rsshub",
    title: "RSSHub GitHub Trending",
    url: "/github/trending/daily/javascript",
    category: "RSSHub",
    configJson: JSON.stringify({ route: "/github/trending/daily/javascript" }),
    enabled: true,
    createdAt: "2026-05-23T10:00:00.000Z",
    lastFetchedAt: null,
    lastError: null,
    articleCount: 0,
    unreadCount: 0,
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
let testModeRssHubSettings: RssHubSettings = {
  ...defaultRssHubSettings,
  instances: defaultRssHubSettings.instances.map((instance) => ({ ...instance })),
};
let testModeInstalledPluginIds = new Set<string>(["official.naixi-forum.xpath"]);
let testModePluginConfigs: Record<string, Record<string, unknown>> = {};
let testModePluginMarkets: PluginMarket[] = [
  {
    id: "official-feaderhub",
    name: "FeaderHub Official",
    repository: "https://github.com/FrankieeW/FeaderHub",
    rawBaseUrl: "https://raw.githubusercontent.com/FrankieeW/FeaderHub/main",
    branch: "main",
    builtin: true,
  },
];
let testModeAutoRefresh: AutoRefreshConfig = {
  enabled: true,
  globalIntervalSeconds: 1800,
  pluginOverrides: [],
  nextRefreshAt: null,
};
let testModeXPathRulePacks: XPathRulePack[] = [
  {
    id: "official.naixi-forum.xpath",
    name: "Naixi Forum XPath Rules",
    version: "0.1.0",
    apiVersion: "xpath-rule-pack/v1",
    kind: "static-xpath-rule-pack",
    registry: "https://github.com/FrankieeW/FeaderHub",
    trust: "official",
    description: "Static XPath rules for naixi.net forum thread lists with section-based browsing.",
    logo: "https://s.naixi.net/favicon.ico",
    capabilities: ["xpath.selectorCandidates"],
    authors: [
      {
        name: "Frankie Wang",
        evmAddress: "0x00000073a2c5581b9ea3d79261a567571Dd14E31",
        avatarUrl: "https://github.com/FrankieeW.png",
        website: "https://github.com/FrankieeW",
        email: "git@frankie.wang",
        githubId: "FrankieeW",
      },
    ],
    candidates: [
      {
        id: "naixi-forum-thread-list",
        pageType: "forum-thread-list",
        priority: 90,
        detect: ["threadlisttableid", "kmlist", "km_subject"],
        selectors: {
          items: "//*[@id='threadlisttableid']/li[contains(@class, 'kmlist')]",
          title: ".//*[contains(@class, 'km_subject')]",
          url: ".//a[contains(@class, 'kmtit')]/@href",
          summary: ".//*[contains(@class, 'kmfoot')]",
          publishedAt: ".//*[contains(@class, 'kmtime')]/*[@title][1]/@title | .//*[contains(@class, 'kmtime')]",
          author: ".//*[contains(@class, 'kmfoot')]/a[starts-with(@href, 'space-uid')][1]",
          cookie: "",
          content: "",
          detailContent: "//*[@id='postlist']//td[contains(@class, 't_f') and starts-with(@id, 'postmessage_')][1]",
          contentCleanup: [
            { pattern: "(?is)<div\\s+class=\"quote\".*?</div>", replacement: "" },
            { pattern: "(?is)<ignore_js_op>.*?</ignore_js_op>", replacement: "" },
          ],
          image: ".//a[contains(@class, 'kmimg')]//img/@src",
          nextPage: "//a[contains(@class, 'nxt')]/@href",
          customFields: [
            { key: "section", label: "Section", xpath: ".//*[contains(@class, 'kmfoot')]/a[contains(@class, 'kmico_bk')][1]", scope: "item" },
            { key: "replies", label: "Replies", xpath: ".//*[contains(@class, 'kmpl')][1]", scope: "item" },
            { key: "views", label: "Views", xpath: ".//*[contains(@class, 'kmck')][1]", scope: "item" },
          ],
          maxItems: 20,
          plugin: undefined,
          reader: {
            removeSelectors: ["//ignore_js_op", "//*[contains(concat(' ', normalize-space(@class), ' '), ' quote ')]"],
            resolveRelativeUrls: true,
            rewriteLinks: true,
            showCustomFields: true,
            layout: { typography: "serif", width: "normal", immersive: false },
          },
        },
      },
    ],
    auth: {
      checkUrl: "https://forum.naixi.net/home.php?mod=spacecp",
      loggedInXpath: "//a[contains(@href,'logout') or contains(@href,'action=logout')]",
    },
    parameters: {
      urlTemplate: "https://forum.naixi.net/{sectionId}.html",
      sections: [
        { id: "forum-64-1", path: ["板块", "内容区", "茶馆", "日常"], url: "https://forum.naixi.net/forum-64-1.html" },
        { id: "forum-64-2", path: ["板块", "内容区", "茶馆", "交易"], url: "https://forum.naixi.net/forum-64-2.html" },
        { id: "forum-64-3", path: ["板块", "内容区", "茶馆", "数科"], url: "https://forum.naixi.net/forum-64-3.html" },
        { id: "forum-65-1", path: ["板块", "内容区", "技术", "建站"], url: "https://forum.naixi.net/forum-65-1.html" },
        { id: "forum-65-2", path: ["板块", "内容区", "技术", "编程"], url: "https://forum.naixi.net/forum-65-2.html" },
        { id: "forum-62-1", path: ["板块", "站务区", "公告"], url: "https://forum.naixi.net/forum-62-1.html" },
      ],
      defaults: { maxItems: 20 },
    },
  },
  {
    id: "official.cyberpunk-ui.view",
    name: "Cyberpunk UI Theme",
    version: "0.1.0",
    apiVersion: "feader-view-plugin/v1",
    kind: "app-ui-theme",
    registry: "https://github.com/FrankieeW/FeaderHub",
    trust: "official",
    description: "Official app-wide cyberpunk theme template for Feader view plugin authors.",
    logo: null,
    capabilities: ["app.theme", "app.chrome", "settings.toggle"],
    candidates: [],
    authors: [
      {
        name: "Frankie Wang",
        evmAddress: "0x00000073a2c5581b9ea3d79261a567571Dd14E31",
        avatarUrl: "https://github.com/FrankieeW.png",
        website: "https://github.com/FrankieeW",
        email: "git@frankie.wang",
        githubId: "FrankieeW",
      },
    ],
  },
  {
    id: "official.social-source-list.view",
    name: "Social Source List View",
    version: "0.1.0",
    apiVersion: "feader-view-plugin/v1",
    kind: "source-list-view",
    registry: "https://github.com/FrankieeW/FeaderHub",
    trust: "official",
    description: "Official post-stream source list template for social media and short-form feeds.",
    logo: null,
    capabilities: ["sourceList.view", "sourceList.avatar", "sourceList.socialMetadata"],
    candidates: [],
    authors: [
      {
        name: "Frankie Wang",
        evmAddress: "0x00000073a2c5581b9ea3d79261a567571Dd14E31",
        avatarUrl: "https://github.com/FrankieeW.png",
        website: "https://github.com/FrankieeW",
        email: "git@frankie.wang",
        githubId: "FrankieeW",
      },
    ],
  },
  {
    id: "official.cinema-detail.view",
    name: "Cinema Detail View",
    version: "0.1.0",
    apiVersion: "feader-view-plugin/v1",
    kind: "detail-view",
    registry: "https://github.com/FrankieeW/FeaderHub",
    trust: "official",
    description: "Official immersive article detail template with cinematic media emphasis.",
    logo: null,
    capabilities: ["detail.view", "detail.heroMedia", "detail.immersiveReading"],
    candidates: [],
    authors: [
      {
        name: "Frankie Wang",
        evmAddress: "0x00000073a2c5581b9ea3d79261a567571Dd14E31",
        avatarUrl: "https://github.com/FrankieeW.png",
        website: "https://github.com/FrankieeW",
        email: "git@frankie.wang",
        githubId: "FrankieeW",
      },
    ],
  },
];

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
    case "add_rsshub_source": {
      const request = args?.request as
        | { route?: string; title?: string; instanceId?: string | null }
        | undefined;
      return upsertTestModeRssHubSource(
        request?.route,
        request?.title,
        request?.instanceId ?? undefined,
      ) as T;
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
    case "get_rsshub_settings":
      return testModeRssHubSettings as T;
    case "set_rsshub_global_instance": {
      const instanceId = String(args?.instanceId ?? "");
      if (testModeRssHubSettings.instances.some((instance) => instance.id === instanceId)) {
        testModeRssHubSettings = { ...testModeRssHubSettings, globalInstanceId: instanceId };
      }
      return testModeRssHubSettings as T;
    }
    case "update_rsshub_source_instance": {
      const request = args?.request as { sourceId?: number; instanceId?: string | null } | undefined;
      const sourceId = Number(request?.sourceId);
      testModeSourceState = testModeSourceState.map((source) => {
        if (source.id !== sourceId || source.kind !== "rsshub") return source;
        const config = readRssHubConfigFromSource(source) ?? { route: source.url };
        return {
          ...source,
          configJson: JSON.stringify({
            ...config,
            instanceId: request?.instanceId || undefined,
          }),
        };
      });
      return testModeSourceState.find((source) => source.id === sourceId) as T;
    }
    case "add_rsshub_instance": {
      const request = args?.request as { name?: string; baseUrl?: string } | undefined;
      const baseUrl = normalizeRssHubBaseUrl(request?.baseUrl ?? "");
      const instance: RssHubInstance = {
        id: instanceIdFromBaseUrl(baseUrl),
        name: request?.name?.trim() || baseUrl.replace(/^https?:\/\//, ""),
        baseUrl,
        maintainer: "Custom",
        location: null,
        official: false,
        builtin: false,
      };
      if (!testModeRssHubSettings.instances.some((item) => item.id === instance.id)) {
        testModeRssHubSettings = {
          ...testModeRssHubSettings,
          instances: [...testModeRssHubSettings.instances, instance],
        };
      }
      return testModeRssHubSettings as T;
    }
    case "check_rsshub_instance": {
      const baseUrl = normalizeRssHubBaseUrl(String(args?.baseUrl ?? ""));
      return {
        ok: true,
        message: "Test mode health check passed",
        checkedUrl: `${baseUrl}/healthz`,
      } as T;
    }
    case "list_plugin_markets":
      return testModePluginMarkets as T;
    case "add_plugin_market": {
      const request = args?.request as { repository?: string; name?: string; branch?: string } | undefined;
      const repo = request?.repository?.trim() || "example/feader-market";
      const repoParts = repo.replace(/^https?:\/\/github.com\//, "").replace(/\.git$/, "").split("/");
      const owner = repoParts[0] || "example";
      const name = repoParts[1] || "feader-market";
      const market: PluginMarket = {
        id: `github-${owner.toLowerCase()}-${name.toLowerCase()}`,
        name: request?.name?.trim() || name,
        repository: `https://github.com/${owner}/${name}`,
        rawBaseUrl: `https://raw.githubusercontent.com/${owner}/${name}/${request?.branch || "main"}`,
        branch: request?.branch || "main",
        builtin: false,
      };
      if (!testModePluginMarkets.some((item) => item.id === market.id)) {
        testModePluginMarkets = [...testModePluginMarkets, market];
      }
      return testModePluginMarkets as T;
    }
    case "install_plugin_from_market": {
      const request = args?.request as { pluginId?: string } | undefined;
      if (request?.pluginId) testModeInstalledPluginIds.add(request.pluginId);
      const pack = testModeXPathRulePacks.find((item) => item.id === request?.pluginId);
      return (pack ? pluginPackFromXPathRulePack(pack) : undefined) as T;
    }
    case "install_plugin_from_url": {
      const pack = {
        ...testModeXPathRulePacks[0],
        id: "direct.example.xpath",
        name: "Direct URL Plugin",
        registry: String((args?.request as { url?: string } | undefined)?.url ?? "direct-url"),
        trust: "direct-url",
      };
      testModeInstalledPluginIds.add(pack.id);
      testModeXPathRulePacks = [pack, ...testModeXPathRulePacks.filter((item) => item.id !== pack.id)];
      return pluginPackFromXPathRulePack(pack) as T;
    }
    case "uninstall_plugin":
      testModeInstalledPluginIds.delete(String(args?.pluginId ?? ""));
      return undefined as T;
    case "get_plugin_config":
      return (testModePluginConfigs[String(args?.pluginId ?? "")] ?? {}) as T;
    case "set_plugin_config": {
      const request = args?.request as { pluginId?: string; values?: Record<string, unknown> } | undefined;
      const pluginId = String(request?.pluginId ?? "");
      testModePluginConfigs[pluginId] = request?.values ?? {};
      return testModePluginConfigs[pluginId] as T;
    }
    case "export_plugin_config": {
      const pluginId = String(args?.pluginId ?? "");
      return JSON.stringify(
        {
          schemaVersion: "feader-plugin-config/v1",
          pluginId,
          exportedAt: new Date().toISOString(),
          values: testModePluginConfigs[pluginId] ?? {},
        },
        null,
        2,
      ) as T;
    }
    case "import_plugin_config": {
      const request = args?.request as { pluginId?: string; json?: string } | undefined;
      const parsed = JSON.parse(request?.json ?? "{}");
      const values = parsed.schemaVersion === "feader-plugin-config/v1" ? parsed.values : parsed;
      testModePluginConfigs[String(request?.pluginId ?? "")] = values;
      return values as T;
    }
    case "list_installed_plugin_packs":
      return testModeXPathRulePacks
        .filter((pack) => testModeInstalledPluginIds.has(pack.id))
        .map(pluginPackFromXPathRulePack) as T;
    case "create_plugin_market_template":
      return {
        path: "/tmp/feader-plugin-market-template",
        files: [
          "registry/index.json",
          "plugins/example/plugin.json",
          "plugins/example/xpath-rule-pack.json",
        ],
      } as T;
    case "list_xpath_plugin_packs":
      return testModeXPathRulePacks as T;
    case "fetch_registry_packs":
      return testModeXPathRulePacks.map((pack) => ({
        ...pluginPackFromXPathRulePack(pack),
        installed: testModeInstalledPluginIds.has(pack.id),
        sourceMarketId: "official-feaderhub",
        sourceMarketName: "FeaderHub Official",
        sourceMarketRepository: "https://github.com/FrankieeW/FeaderHub",
      })) as T;
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
    case "preview_json_api_source":
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
    case "update_xpath_source": {
      const request = args?.request as { sourceId?: number; selectors?: XPathSelectors } | undefined;
      const sourceId = Number(request?.sourceId);
      const selectors = compactXPathSelectors(request?.selectors ?? defaultXPathSelectors);
      testModeSourceState = testModeSourceState.map((source) =>
        source.id === sourceId && source.kind === "xpath"
          ? {
              ...source,
              configJson: JSON.stringify(selectors),
              lastFetchedAt: new Date().toISOString(),
              lastError: null,
            }
          : source,
      );
      return testModeSourceState.find((source) => source.id === sourceId) as T;
    }
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
    case "get_plugin_credential":
      return { pluginId: String(args?.pluginId ?? ""), cookieSet: false } as T;
    case "set_plugin_credential":
      return { pluginId: String(args?.pluginId ?? ""), cookieSet: Boolean(String(args?.cookie ?? "").trim()) } as T;
    case "check_plugin_credential":
      return { ok: true, message: "测试模式:已登录", checkedAt: new Date().toISOString() } as T;
    case "get_auto_refresh_config":
      return testModeAutoRefresh as T;
    case "set_global_refresh_interval": {
      const seconds = Number(args?.seconds);
      testModeAutoRefresh = { ...testModeAutoRefresh, globalIntervalSeconds: seconds };
      return testModeAutoRefresh as T;
    }
    case "set_plugin_refresh_interval": {
      const pluginId = String(args?.pluginId ?? "");
      const seconds = Number(args?.seconds);
      testModeAutoRefresh = {
        ...testModeAutoRefresh,
        pluginOverrides: [
          ...testModeAutoRefresh.pluginOverrides.filter((o) => o.pluginId !== pluginId),
          { pluginId, pluginName: pluginId, refreshIntervalSeconds: seconds },
        ],
      };
      return testModeAutoRefresh as T;
    }
    case "set_source_refresh_interval": {
      const sourceId = Number(args?.sourceId);
      const seconds = args?.seconds != null ? Number(args.seconds) : null;
      testModeSourceState = testModeSourceState.map((source) =>
        source.id === sourceId ? { ...source, refreshIntervalSeconds: seconds } : source,
      );
      return testModeAutoRefresh as T;
    }
    case "set_auto_refresh_enabled": {
      const enabled = Boolean(args?.enabled);
      testModeAutoRefresh = { ...testModeAutoRefresh, enabled };
      return testModeAutoRefresh as T;
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

function upsertTestModeRssHubSource(
  route = "/github/trending/daily/javascript",
  title = "RSSHub source",
  instanceId?: string | null,
): Source {
  const normalizedRoute = normalizeRssHubRoute(route);
  const existing = testModeSourceState.find(
    (source) => source.kind === "rsshub" && source.url === normalizedRoute,
  );
  if (existing) {
    return existing;
  }

  const config: RssHubSourceConfig = {
    route: normalizedRoute,
    instanceId: instanceId || undefined,
  };
  const source: Source = {
    id: Math.max(0, ...testModeSourceState.map((item) => item.id)) + 1,
    kind: "rsshub",
    title: title.trim() || normalizedRoute,
    url: normalizedRoute,
    category: "RSSHub",
    configJson: JSON.stringify(config),
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

function normalizeRssHubRoute(route: string): string {
  const trimmed = route.trim();
  if (!trimmed) return "/github/trending/daily/javascript";
  try {
    if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
      const parsed = new URL(trimmed);
      return `${parsed.pathname}${parsed.search}`;
    }
  } catch {
    return trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
  }
  return trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
}

function normalizeRssHubBaseUrl(baseUrl: string): string {
  const trimmed = baseUrl.trim().replace(/\/+$/, "");
  return trimmed || "https://rsshub.rssforever.com";
}

function instanceIdFromBaseUrl(baseUrl: string): string {
  return baseUrl
    .replace(/^https?:\/\//, "")
    .replace(/\/+$/, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

const uncategorizedLabel = "Uncategorized";

const REFRESH_INTERVAL_PRESETS = [
  { label: "15m", seconds: 900 },
  { label: "30m", seconds: 1800 },
  { label: "1h", seconds: 3600 },
  { label: "2h", seconds: 7200 },
  { label: "6h", seconds: 21600 },
  { label: "12h", seconds: 43200 },
  { label: "24h", seconds: 86400 },
];

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
  const [pendingDelete, setPendingDelete] = useState<{ id: number; title: string } | null>(null);
  const [articles, setArticles] = useState<Article[]>([]);
  const [selectedSourceId, setSelectedSourceId] = useState<number | undefined>();
  const [selectedManagerSourceId, setSelectedManagerSourceId] = useState<number | undefined>();
  const [selectedArticleId, setSelectedArticleId] = useState<number | undefined>();
  const [readerView, setReaderView] = useState<ReaderView>("none");
  const [filterMode, setFilterMode] = useState<FilterMode>("all");
  const [sourceInputMode, setSourceInputMode] = useState<SourceInputMode>("rss");
  const [activeView, setActiveView] = useState<ViewMode>("reader");
  const [showSourceComposer, setShowSourceComposer] = useState(false);
  const [themeMode, setThemeMode] = useState<ThemeMode>(() => readInitialThemeMode());
  const [entryLayout, setEntryLayout] = useState<EntryLayout>(() => readInitialEntryLayout());
  const [appUiThemeByMode, setAppUiThemeByMode] = useState<AppUiThemeByMode>(() =>
    readInitialAppUiThemeByMode(),
  );
  const [detailViewPlugin, setDetailViewPlugin] = useState<DetailViewPluginId | null>(() =>
    readInitialPluginId(detailViewPluginStorageKey, detailViewPlugins),
  );
  const [installedViewPlugins, setInstalledViewPlugins] = useState<string[]>(() =>
    readInitialInstalledViewPlugins(),
  );
  const [installedViewPluginVersions, setInstalledViewPluginVersions] = useState<
    Record<string, string>
  >(() => readInitialInstalledViewPluginVersions());
  const [sourceListViewBySource, setSourceListViewBySource] = useState<Record<string, string>>(() =>
    readInitialSourceListViewBySource(),
  );
  const [readerTypography, setReaderTypography] = useState<ReaderTypography>(() =>
    readInitialReaderTypography(),
  );
  const [paneWidths, setPaneWidths] = useState<PaneWidths>(() => readInitialPaneWidths());
  const [feedUrl, setFeedUrl] = useState("");
  const [rssHubTitle, setRssHubTitle] = useState("");
  const [rssHubInstanceId, setRssHubInstanceId] = useState("");
  const [rssHubStatus, setRssHubStatus] = useState<string | null>(null);
  const [rssHubSettings, setRssHubSettings] = useState<RssHubSettings>(defaultRssHubSettings);
  const [xpathTitle, setXPathTitle] = useState("");
  const [xpathSelectors, setXPathSelectors] = useState<XPathSelectors>(defaultXPathSelectors);
  const [xpathPreview, setXPathPreview] = useState<XPathPreview | null>(null);
  const [xpathStatus, setXPathStatus] = useState<string | null>(null);
  const [editingXPathSourceId, setEditingXPathSourceId] = useState<number | null>(null);
  const [editXPathSelectors, setEditXPathSelectors] =
    useState<XPathSelectors>(defaultXPathSelectors);
  const [editXPathPreview, setEditXPathPreview] = useState<XPathPreview | null>(null);
  const [editXPathStatus, setEditXPathStatus] = useState<string | null>(null);
  const [aiSettings, setAiSettings] = useState<AiSettings>(defaultAiSettings);
  const [xpathRulePacks, setXPathRulePacks] = useState<MarketplacePluginPack[]>([]);
  const [pluginMarkets, setPluginMarkets] = useState<PluginMarket[]>([]);
  const [pluginMarketRepository, setPluginMarketRepository] = useState("");
  const [pluginMarketName, setPluginMarketName] = useState("");
  const [pluginInstallUrl, setPluginInstallUrl] = useState("");
  const [pluginTemplateStatus, setPluginTemplateStatus] = useState<string | null>(null);
  const [hubSearchQuery, setHubSearchQuery] = useState("");
  const [hubInstallFilter, setHubInstallFilter] = useState<HubInstallFilter>("all");
  const [hubGroup, setHubGroup] = useState<"all" | "sources" | "appearance">("all");
  const [hubCategory, setHubCategory] = useState("all");
  const [showPluginDialog, setShowPluginDialog] = useState<PluginPack | null>(null);
  const [dialogUrl, setDialogUrl] = useState("");
  const [dialogTitle, setDialogTitle] = useState("");
  const [dialogSectionId, setDialogSectionId] = useState("");
  const [dialogCandidateId, setDialogCandidateId] = useState("");
  const [dialogMaxItems, setDialogMaxItems] = useState<number | undefined>();
  const [dialogCookie, setDialogCookie] = useState("");
  const [pluginCredential, setPluginCredential] = useState<PluginCredential | null>(null);
  const [credentialCheck, setCredentialCheck] = useState<CredentialCheck | null>(null);
  const [dialogPreview, setDialogPreview] = useState<XPathPreview | null>(null);
  const [dialogStatus, setDialogStatus] = useState<string | null>(null);
  const [isDialogBusy, setIsDialogBusy] = useState(false);
  const [dialogParamValues, setDialogParamValues] = useState<Record<string, string>>({});
  const [hubRegistryStatus, setHubRegistryStatus] = useState<string | null>(null);
  const [walletSession, setWalletSession] = useState<WalletSession | null>(null);
  const [status, setStatus] = useState("Ready");
  const [isBusy, setIsBusy] = useState(false);
  const [autoRefreshConfig, setAutoRefreshConfig] = useState<AutoRefreshConfig | null>(null);
  const [refreshTick, setRefreshTick] = useState<RefreshTickEvent | null>(null);

  const selectedSource = useMemo(
    () => sources.find((source) => source.id === selectedSourceId),
    [selectedSourceId, sources],
  );
  const selectedManagerSource = useMemo(
    () => sources.find((source) => source.id === selectedManagerSourceId) ?? sources[0],
    [selectedManagerSourceId, sources],
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
    void loadRssHubSettings();
    void loadPluginMarkets();
    void loadXPathPluginPacks();
    void loadAutoRefreshConfig();
    // Listen for refresh-tick events from backend scheduler
    let unlisten: (() => void) | undefined;
    if (isTauriRuntime()) {
      import("@tauri-apps/api/event")
        .then(({ listen }) => {
          listen<RefreshTickEvent>("refresh-tick", (event) => {
            setRefreshTick(event.payload);
          }).then((fn) => {
            unlisten = fn;
          });
        })
        .catch(() => {});
    }
    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const applyAppearance = () => {
      applyThemeMode(themeMode);
      applyAppUiPlugin(appUiThemeByMode[resolveThemeMode(themeMode)]);
    };
    applyAppearance();
    localStorage.setItem(themeStorageKey, themeMode);

    if (themeMode !== "system") {
      return;
    }

    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", applyAppearance);
    return () => media.removeEventListener("change", applyAppearance);
  }, [themeMode, appUiThemeByMode]);

  useEffect(() => {
    localStorage.setItem(entryLayoutStorageKey, entryLayout);
  }, [entryLayout]);

  useEffect(() => {
    localStorage.setItem(appUiThemeByModeStorageKey, JSON.stringify(appUiThemeByMode));
  }, [appUiThemeByMode]);

  useEffect(() => {
    persistNullablePlugin(detailViewPluginStorageKey, detailViewPlugin);
  }, [detailViewPlugin]);

  useEffect(() => {
    localStorage.setItem(installedViewPluginsStorageKey, JSON.stringify(installedViewPlugins));
    setAppUiThemeByMode((current) => {
      const next: AppUiThemeByMode = {
        light: current.light && !installedViewPlugins.includes(current.light) ? null : current.light,
        dark: current.dark && !installedViewPlugins.includes(current.dark) ? null : current.dark,
      };
      return next.light === current.light && next.dark === current.dark ? current : next;
    });
    setDetailViewPlugin((current) =>
      current && !installedViewPlugins.includes(current) ? null : current,
    );
  }, [installedViewPlugins]);

  useEffect(() => {
    localStorage.setItem(
      installedViewPluginVersionsStorageKey,
      JSON.stringify(installedViewPluginVersions),
    );
  }, [installedViewPluginVersions]);

  useEffect(() => {
    localStorage.setItem(sourceListViewBySourceStorageKey, JSON.stringify(sourceListViewBySource));
  }, [sourceListViewBySource]);

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

  const hubGroups = useMemo(() => {
    let hasSources = false;
    let hasAppearance = false;
    for (const pack of xpathRulePacks) {
      if (isViewPluginPack(pack)) hasAppearance = true;
      else hasSources = true;
    }
    const groups: { id: "all" | "sources" | "appearance"; label: string }[] = [
      { id: "all", label: "All" },
    ];
    if (hasSources) groups.push({ id: "sources", label: "Sources" });
    if (hasAppearance) groups.push({ id: "appearance", label: "Appearance" });
    return groups;
  }, [xpathRulePacks]);

  const hubSubCategories = useMemo(() => {
    const present = new Set<string>();
    if (hubGroup === "sources") {
      for (const pack of xpathRulePacks) {
        if (isViewPluginPack(pack)) continue;
        for (const family of sourcePackFamilies(pack)) present.add(family);
      }
      return sourceFamilyOrder.filter((family) => present.has(family));
    }
    if (hubGroup === "appearance") {
      for (const pack of xpathRulePacks) {
        const slot = viewPluginCategory(pack);
        if (slot) present.add(slot);
      }
      return viewSlotOrder.filter((slot) => present.has(slot));
    }
    return [];
  }, [xpathRulePacks, hubGroup]);

  const filteredPacks = useMemo(() => {
    const query = hubSearchQuery.trim().toLowerCase();
    return xpathRulePacks.filter((pack) => {
      const isView = isViewPluginPack(pack);
      if (hubGroup === "sources" && isView) return false;
      if (hubGroup === "appearance" && !isView) return false;
      if (hubCategory !== "all") {
        const matchesCat = isView
          ? viewPluginCategory(pack) === hubCategory
          : sourcePackFamilies(pack).has(hubCategory);
        if (!matchesCat) return false;
      }
      const packInstalled = isViewPluginPack(pack)
        ? installedViewPlugins.includes(viewPluginIdFromPack(pack))
        : pack.installed;
      if (hubInstallFilter === "installed" && !packInstalled) return false;
      if (hubInstallFilter === "not-installed" && packInstalled) return false;
      if (hubInstallFilter === "needs-update") {
        if (!packInstalled) return false;
        const installedVersion = isViewPluginPack(pack)
          ? installedViewPluginVersions[viewPluginIdFromPack(pack)] ?? null
          : pack.installedVersion ?? null;
        if (!installedVersion) {
          if (!(isViewPluginPack(pack) && Boolean(pack.sourceMarketId))) return false;
        } else if (comparePluginVersions(pack.version, installedVersion) <= 0) {
          return false;
        }
      }
      if (!query) return true;
      return (
        pack.name.toLowerCase().includes(query) ||
        pack.description.toLowerCase().includes(query) ||
        pack.kind.toLowerCase().includes(query) ||
        pack.capabilities.some((cap) => cap.toLowerCase().includes(query))
      );
    });
  }, [xpathRulePacks, hubSearchQuery, hubGroup, hubCategory, hubInstallFilter, installedViewPlugins, installedViewPluginVersions]);

  const officialPacks = useMemo(
    () => filteredPacks.filter((p) => p.trust === "bundled-official" || p.trust === "official"),
    [filteredPacks],
  );

  const communityPacks = useMemo(
    () => filteredPacks.filter((p) => p.trust !== "bundled-official" && p.trust !== "official"),
    [filteredPacks],
  );

  useEffect(() => {
    if (activeView !== "sources") {
      return;
    }
    if (selectedManagerSourceId && sources.some((source) => source.id === selectedManagerSourceId)) {
      return;
    }
    setSelectedManagerSourceId(sources[0]?.id);
  }, [activeView, selectedManagerSourceId, sources]);

  async function loadAutoRefreshConfig(): Promise<void> {
    try {
      const config = await invoke<AutoRefreshConfig>("get_auto_refresh_config");
      setAutoRefreshConfig(config);
    } catch {
      // Silently use defaults when backend is unavailable
    }
  }

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

  async function loadRssHubSettings(): Promise<void> {
    const settings = await invoke<RssHubSettings>("get_rsshub_settings");
    setRssHubSettings(settings);
    setRssHubInstanceId((current) => current || "");
  }

  async function loadXPathPluginPacks(forceRefresh = false): Promise<void> {
    try {
      const packs = await invoke<MarketplacePluginPack[]>("fetch_registry_packs", {
        forceRefresh,
      });
      setXPathRulePacks(withBuiltinViewPacks(packs));
      setHubRegistryStatus(forceRefresh ? "Remote registry refreshed." : "Remote registry loaded.");
    } catch (error) {
      const packs = await invoke<XPathRulePack[]>("list_xpath_plugin_packs");
      setXPathRulePacks(
        withBuiltinViewPacks(
          packs.map((pack) => ({
            ...pluginPackFromXPathRulePack(pack),
            installed: true,
            sourceMarketId: null,
          })),
        ),
      );
      setHubRegistryStatus(`Remote registry unavailable. Showing bundled packs. ${String(error)}`);
    }
  }

  async function loadPluginMarkets(): Promise<void> {
    const markets = await invoke<PluginMarket[]>("list_plugin_markets");
    setPluginMarkets(markets);
  }

  function pluginUrlFromParams(pack: PluginPack, params: Record<string, string>): string {
    let url = pluginParameters(pack)?.urlTemplate ?? "";
    for (const [key, value] of Object.entries(params)) {
      url = url.split(`{${key}}`).join(encodeURIComponent(value));
    }
    return url;
  }

  function openPluginDialog(pack: PluginPack): void {
    const xpath = pack.xpath;
    if (!xpath) {
      setActiveView(pack.runtime ? "settings" : activeView);
      setStatus(
        pack.runtime
          ? `${pack.name} is a runtime source plugin. Configure its runtime in RSSHub settings before adding RSSHub routes.`
          : `${pack.name} is not a source-rule plugin.`,
      );
      return;
    }
    const sections = xpath.parameters?.sections;
    const firstSection = sections?.[0];
    const firstCandidate = xpath.candidates[0];
    const pluginParams = xpath.parameters?.params;

    // Initialize param values from plugin param defaults
    const initialParams: Record<string, string> = {};
    if (pluginParams) {
      for (const param of pluginParams) {
        if (param.type === "select" && param.options?.[0]) {
          initialParams[param.key] = param.options[0].value;
        } else {
          initialParams[param.key] = "";
        }
      }
      setDialogParamValues(initialParams);
      // Auto-construct URL from params
      setDialogUrl(pluginUrlFromParams(pack, initialParams));
    } else {
      setDialogParamValues({});
      setDialogUrl(firstSection?.url ?? "");
    }

    setDialogSectionId(firstSection?.id ?? "");
    setDialogTitle(pluginSourceTitle(pack, firstSection));
    setDialogCandidateId(firstCandidate?.id ?? "");
    setDialogMaxItems(firstCandidate?.selectors.maxItems ?? xpath.parameters?.defaults?.maxItems);
    setDialogCookie(firstCandidate?.selectors.cookie ?? "");
    setDialogPreview(null);
    setDialogStatus(null);
    setCredentialCheck(null);
    setShowPluginDialog(pack);
    invoke<PluginCredential>("get_plugin_credential", { pluginId: pack.id })
      .then(setPluginCredential)
      .catch(() => setPluginCredential(null));
  }

  function closePluginDialog(): void {
    setShowPluginDialog(null);
    setDialogPreview(null);
    setDialogStatus(null);
  }

  function dialogSelectors(): XPathSelectors | null {
    if (!showPluginDialog) return null;
    const xpath = showPluginDialog.xpath;
    if (!xpath) return null;
    const candidate =
      xpath.candidates.find((item) => item.id === dialogCandidateId) ?? xpath.candidates[0];
    if (!candidate) return null;
    return {
      ...candidate.selectors,
      cookie: dialogCookie.trim() || undefined,
      maxItems: dialogMaxItems,
      plugin: pluginSourceInfo(showPluginDialog, candidate),
    };
  }

  async function handleDialogPreview(): Promise<void> {
    const url = dialogUrl.trim();
    const selectors = dialogSelectors();
    if (!url || !selectors) {
      setDialogStatus("Enter a URL first.");
      return;
    }
    setDialogStatus("Previewing...");
    setIsDialogBusy(true);
    try {
      const command =
        showPluginDialog?.kind === "json-api-feed" ? "preview_json_api_source" : "preview_xpath_source";
      const preview = await invoke<XPathPreview>(command, {
        request: { url, selectors },
      });
      setDialogPreview(preview);
      setDialogStatus(`Preview: ${preview.articles.length} articles found`);
    } catch (error) {
      setDialogStatus(String(error));
      setDialogPreview(null);
    } finally {
      setIsDialogBusy(false);
    }
  }

  async function handleDialogAddSource(): Promise<void> {
    const url = dialogUrl.trim();
    const title = dialogTitle.trim();
    const selectors = dialogSelectors();
    if (!url || !title || !selectors) {
      setDialogStatus("URL and title are required.");
      return;
    }
    setDialogStatus("Adding source...");
    setIsDialogBusy(true);
    try {
      const command = showPluginDialog?.kind === "json-api-feed" ? "add_json_api_source" : "add_xpath_source";
      await invoke<Source>(command, {
        request: { url, title, selectors },
      });
      closePluginDialog();
      setActiveView("reader");
      await loadData(undefined, "all", undefined);
      setStatus(`Added source: ${title}`);
    } catch (error) {
      setDialogStatus(String(error));
    } finally {
      setIsDialogBusy(false);
    }
  }

  async function handleInstallPlugin(pack: MarketplacePluginPack): Promise<void> {
    if (isViewPluginPack(pack)) {
      handleInstallViewPlugin(pack);
      return;
    }
    if (!pack.sourceMarketId) {
      setStatus("Bundled plugin is already installed");
      return;
    }
    await runTask("Installing plugin", async () => {
      await invoke<PluginPack>("install_plugin_from_market", {
        request: { marketId: pack.sourceMarketId, pluginId: pack.id },
      });
      await loadXPathPluginPacks(false);
      setStatus(`Installed plugin: ${pack.name}`);
    });
  }

  async function handleUpdatePlugin(pack: MarketplacePluginPack): Promise<void> {
    if (isViewPluginPack(pack)) {
      handleInstallViewPlugin(pack);
      setStatus(`Updated view plugin: ${pack.name}`);
      return;
    }
    if (!pack.sourceMarketId) {
      setStatus("Bundled plugin is already current");
      return;
    }
    await runTask("Updating plugin", async () => {
      await invoke<PluginPack>("install_plugin_from_market", {
        request: { marketId: pack.sourceMarketId, pluginId: pack.id },
      });
      await loadXPathPluginPacks(false);
      setStatus(`Updated plugin: ${pack.name}`);
    });
  }

  async function handleUninstallPlugin(pack: MarketplacePluginPack): Promise<void> {
    if (isViewPluginPack(pack)) {
      handleUninstallViewPlugin(pack);
      return;
    }
    await runTask("Uninstalling plugin", async () => {
      await invoke<void>("uninstall_plugin", { pluginId: pack.id });
      await loadXPathPluginPacks(false);
      setStatus(`Uninstalled plugin: ${pack.name}`);
    });
  }

  function handleInstallViewPlugin(pack: MarketplacePluginPack): void {
    const pluginId = viewPluginIdFromPack(pack);
    setInstalledViewPlugins((current) =>
      current.includes(pluginId) ? current : [...current, pluginId],
    );
    setInstalledViewPluginVersions((current) => ({ ...current, [pluginId]: pack.version }));
    setStatus(`Installed view plugin: ${pack.name}`);
  }

  function handleUninstallViewPlugin(pack: MarketplacePluginPack): void {
    const pluginId = viewPluginIdFromPack(pack);
    setInstalledViewPlugins((current) => current.filter((id) => id !== pluginId));
    setInstalledViewPluginVersions((current) => {
      const { [pluginId]: _removed, ...next } = current;
      return next;
    });
    setStatus(`Uninstalled view plugin: ${pack.name}`);
  }

  function isViewPluginInstalled(pack: MarketplacePluginPack): boolean {
    return installedViewPlugins.includes(viewPluginIdFromPack(pack));
  }

  function pluginActionLabel(pack: MarketplacePluginPack): string {
    if (isViewPluginPack(pack)) {
      return isViewPluginInstalled(pack) ? "Uninstall" : "Install";
    }
    if (isRuntimeSourcePluginPack(pack)) {
      return pack.installed ? "Configure" : "Install";
    }
    return pack.installed ? "Add Source" : "Install";
  }

  function pluginInstalledVersion(pack: MarketplacePluginPack): string | null {
    if (isViewPluginPack(pack)) {
      return installedViewPluginVersions[viewPluginIdFromPack(pack)] ?? null;
    }
    return pack.installedVersion ?? null;
  }

  function pluginHasUpdate(pack: MarketplacePluginPack): boolean {
    const installedVersion = pluginInstalledVersion(pack);
    if (!installedVersion) {
      return isViewPluginPack(pack) && isViewPluginInstalled(pack) && Boolean(pack.sourceMarketId);
    }
    return comparePluginVersions(pack.version, installedVersion) > 0;
  }

  async function handleAddPluginMarket(): Promise<void> {
    await runTask("Adding plugin market", async () => {
      const markets = await invoke<PluginMarket[]>("add_plugin_market", {
        request: {
          repository: pluginMarketRepository,
          name: pluginMarketName || null,
          branch: "main",
        },
      });
      setPluginMarkets(markets);
      setPluginMarketRepository("");
      setPluginMarketName("");
      await loadXPathPluginPacks(true);
      setStatus("Plugin market added");
    }, setHubRegistryStatus);
  }

  async function handleInstallPluginUrl(): Promise<void> {
    await runTask("Installing plugin URL", async () => {
      const pack = await invoke<PluginPack>("install_plugin_from_url", {
        request: { url: pluginInstallUrl },
      });
      setPluginInstallUrl("");
      await loadXPathPluginPacks(false);
      setStatus(`Installed plugin: ${pack.name}`);
    }, setHubRegistryStatus);
  }

  async function handleCreatePluginMarketTemplate(): Promise<void> {
    await runTask("Creating plugin market template", async () => {
      const template = await invoke<PluginMarketTemplate>("create_plugin_market_template");
      setPluginTemplateStatus(`Template created at ${template.path}`);
      setStatus(`Plugin market template created: ${template.path}`);
    }, setPluginTemplateStatus);
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

  async function handleSetRssHubGlobalInstance(instanceId: string): Promise<void> {
    await runTask("Saving RSSHub settings", async () => {
      const next = await invoke<RssHubSettings>("set_rsshub_global_instance", { instanceId });
      setRssHubSettings(next);
      setStatus("RSSHub global instance updated");
    });
  }

  async function handleAddRssHubInstance(name: string, baseUrl: string): Promise<void> {
    await runTask("Adding RSSHub instance", async () => {
      const next = await invoke<RssHubSettings>("add_rsshub_instance", {
        request: { name, baseUrl },
      });
      setRssHubSettings(next);
      setStatus("RSSHub instance added");
    });
  }

  async function handleCheckRssHubInstance(baseUrl: string): Promise<void> {
    await runTask("Checking RSSHub instance", async () => {
      const result = await invoke<RssHubInstanceCheck>("check_rsshub_instance", { baseUrl });
      setRssHubStatus(result.message);
      setStatus(`${result.ok ? "RSSHub available" : "RSSHub check failed"}: ${result.checkedUrl}`);
    }, setRssHubStatus);
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
    } else if (sourceInputMode === "rsshub") {
      setRssHubStatus("Confirming RSSHub route...");
    }
    await runTask("Adding feed", async () => {
      const source =
        sourceInputMode === "rss"
          ? await invoke<Source>("add_source", { request: { url } })
          : sourceInputMode === "rsshub"
            ? await invoke<Source>("add_rsshub_source", {
                request: {
                  route: url,
                  title: rssHubTitle,
                  instanceId: rssHubInstanceId || null,
                },
              })
            : await invoke<Source>("add_xpath_source", {
              request: {
                url,
                title: xpathTitle,
                selectors: compactXPathSelectors(xpathSelectors),
              },
            });
      setFeedUrl("");
      setRssHubTitle("");
      setRssHubInstanceId("");
      setRssHubStatus(null);
      setXPathTitle("");
      setXPathPreview(null);
      setXPathStatus(null);
      setShowSourceComposer(false);
      setSelectedSourceId(source.id);
      setSelectedManagerSourceId(source.id);
      setFilterMode("all");
      await loadData(source.id, "all", undefined);
      setStatus(`Added ${source.title}`);
    }, sourceInputMode === "xpath" ? setXPathStatus : sourceInputMode === "rsshub" ? setRssHubStatus : undefined);
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

  function handleStartEditXPathSource(source: Source): void {
    setEditingXPathSourceId(source.id);
    setEditXPathSelectors(readXPathSelectorsFromSource(source));
    setEditXPathPreview(null);
    setEditXPathStatus(null);
  }

  async function handlePreviewXPathEdit(source: Source): Promise<void> {
    setEditXPathStatus("Previewing XPath selectors...");
    await runTask("Previewing XPath", async () => {
      const preview = await invoke<XPathPreview>("preview_xpath_source", {
        request: {
          url: source.url,
          selectors: compactXPathSelectors(editXPathSelectors),
        },
      });
      setEditXPathPreview(preview);
      const message = `Preview extracted ${preview.articles.length} articles`;
      setStatus(message);
      setEditXPathStatus(message);
    }, setEditXPathStatus);
  }

  async function handleSaveXPathEdit(source: Source): Promise<void> {
    setEditXPathStatus("Saving XPath selectors...");
    await runTask("Saving XPath source", async () => {
      const updated = await invoke<Source>("update_xpath_source", {
        request: {
          sourceId: source.id,
          selectors: compactXPathSelectors(editXPathSelectors),
        },
      });
      setEditingXPathSourceId(null);
      setEditXPathPreview(null);
      setEditXPathStatus(null);
      await loadData(selectedSourceId, filterMode, selectedArticleId);
      setSelectedManagerSourceId(updated.id);
      setStatus(`Updated XPath selectors for ${updated.title}`);
    }, setEditXPathStatus);
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

  function requestDeleteSource(sourceId: number, title: string): void {
    setPendingDelete({ id: sourceId, title });
  }

  async function handleDeleteSourceId(sourceId: number, title: string): Promise<void> {
    const remainingSources = sources.filter((source) => source.id !== sourceId);
    const nextManagerSourceId = remainingSources[0]?.id;
    const nextReaderSourceId = selectedSourceId === sourceId ? undefined : selectedSourceId;
    const nextArticleId = selectedSourceId === sourceId ? undefined : selectedArticleId;

    await runTask("Deleting feed", async () => {
      await invoke("delete_source", { sourceId });
      setSources(remainingSources);
      setArticles((current) =>
        selectedSourceId === sourceId
          ? []
          : current.filter((article) => article.sourceId !== sourceId),
      );
      setSelectedManagerSourceId(nextManagerSourceId);
      if (editingXPathSourceId === sourceId) {
        setEditingXPathSourceId(null);
        setEditXPathPreview(null);
        setEditXPathStatus(null);
      }
      if (selectedSourceId === sourceId) {
        setSelectedSourceId(undefined);
        setSelectedArticleId(undefined);
      }
      await loadData(nextReaderSourceId, filterMode, nextArticleId);
      setStatus(`Deleted "${title}"`);
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
    setAppUiThemeByMode({ light: null, dark: null });
    setDetailViewPlugin(null);
    setSourceListViewBySource({});
    localStorage.removeItem(paneStorageKey);
    localStorage.removeItem(entryLayoutStorageKey);
    localStorage.removeItem(appUiThemeByModeStorageKey);
    localStorage.removeItem(detailViewPluginStorageKey);
    localStorage.removeItem(sourceListViewBySourceStorageKey);
  }

  async function handleToggleAutoRefresh(enabled: boolean): Promise<void> {
    const config = await invoke<AutoRefreshConfig>("set_auto_refresh_enabled", { enabled });
    setAutoRefreshConfig(config);
    setStatus(enabled ? "Auto-refresh enabled" : "Auto-refresh paused");
  }

  async function handleSetGlobalRefreshInterval(seconds: number): Promise<void> {
    const config = await invoke<AutoRefreshConfig>("set_global_refresh_interval", { seconds });
    setAutoRefreshConfig(config);
    setStatus(`Global refresh interval set to ${formatInterval(seconds)}`);
  }

  async function handleSetPluginRefreshInterval(pluginId: string, seconds: number): Promise<void> {
    const config = await invoke<AutoRefreshConfig>("set_plugin_refresh_interval", {
      pluginId,
      seconds,
    });
    setAutoRefreshConfig(config);
  }

  async function handleSetSourceRefreshInterval(
    sourceId: number,
    seconds: number | null,
  ): Promise<void> {
    const config = await invoke<AutoRefreshConfig>("set_source_refresh_interval", {
      sourceId,
      seconds,
    });
    setAutoRefreshConfig(config);
    await loadData(selectedSourceId, filterMode, selectedArticleId);
  }

  async function handleSetSourceRssHubInstance(
    sourceId: number,
    instanceId: string | null,
  ): Promise<void> {
    await runTask("Updating RSSHub source", async () => {
      await invoke<Source>("update_rsshub_source_instance", {
        request: { sourceId, instanceId },
      });
      await loadData(selectedSourceId, filterMode, selectedArticleId);
      setStatus("RSSHub source instance updated");
    });
  }

  function formatInterval(seconds: number): string {
    if (seconds < 120) return `${seconds}s`;
    if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
    if (seconds < 86400) return `${(seconds / 3600).toFixed(1).replace(/\.0$/, "")}h`;
    return `${(seconds / 86400).toFixed(1).replace(/\.0$/, "")}d`;
  }

  function formatCountdown(isoString: string | null | undefined): string {
    if (!isoString) return "";
    const diff = Math.max(0, (Date.parse(isoString) - Date.now()) / 1000);
    if (diff < 60) return "soon";
    if (diff < 3600) return `${Math.round(diff / 60)}m`;
    return `${(diff / 3600).toFixed(1).replace(/\.0$/, "")}h`;
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

  const currentSourceKey = String(selectedSourceId ?? "all");
  const storedListView = sourceListViewBySource[currentSourceKey] ?? entryLayout;
  const effectiveListView =
    isSourceListPluginId(storedListView) && !installedViewPlugins.includes(storedListView)
      ? "list"
      : storedListView;
  const appUiPluginOptions = useMemo(
    () => viewPluginDefinitionsForKind(xpathRulePacks, "app-ui-theme"),
    [xpathRulePacks],
  );
  const installedRuntimePlugins = useMemo(
    () =>
      xpathRulePacks.filter(
        (pack) => isRuntimeSourcePluginPack(pack) && pack.installed && pack.runtime?.settingsPage,
      ),
    [xpathRulePacks],
  );
  const installedSourceListPlugins = useMemo(
    () => sourceListPlugins.filter((plugin) => installedViewPlugins.includes(plugin.id)),
    [installedViewPlugins],
  );
  const resolvedThemeMode = resolveThemeMode(themeMode);
  const resolvedAppUiTheme = appUiThemeByMode[resolvedThemeMode];
  const resolvedAppUiPlugin = appUiPluginOptions.find((plugin) => plugin.id === resolvedAppUiTheme);
  const shellStyle = {
    "--sidebar-width": `${paneWidths.sidebar}px`,
    "--timeline-width": `${paneWidths.timeline}px`,
    ...cssVariablesFromAppUiTokens(resolvedAppUiPlugin?.tokens),
  } as CSSProperties;

  function handleSelectListView(choice: string): void {
    setSourceListViewBySource((current) => ({ ...current, [currentSourceKey]: choice }));
  }

  function handleAssignTheme(mode: "light" | "dark", pluginId: AppUiPluginId | null): void {
    setAppUiThemeByMode((current) => ({ ...current, [mode]: pluginId }));
  }

  return (
    <main
      className="app-shell"
      data-app-ui-plugin={resolvedAppUiTheme ?? "native"}
      data-view={activeView}
      style={shellStyle}
    >
      <IconRail
        activeView={activeView}
        onSelectView={setActiveView}
        themeMode={themeMode}
        onToggleTheme={() => setThemeMode((mode) => oppositeResolvedThemeMode(mode))}
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
            <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
              <button
                className="secondary-action"
                style={{ flex: 1 }}
                disabled={isBusy}
                onClick={handleRefreshAll}
                type="button"
              >
                Refresh all sources
              </button>
              {autoRefreshConfig?.enabled && refreshTick?.nextRefreshAt ? (
                <span className="badge" title="Next auto-refresh" style={{ fontSize: "0.75rem", opacity: 0.7, whiteSpace: "nowrap" }}>
                  {formatCountdown(refreshTick.nextRefreshAt)}
                </span>
              ) : null}
            </div>

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
                          <div className="feed-row" key={source.id}>
                            <button
                              className={`feed-item ${selectedSourceId === source.id ? "active" : ""}`}
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
                            <button
                              aria-label={`Delete ${source.title}`}
                              className="feed-delete"
                              disabled={isBusy}
                              onClick={() => requestDeleteSource(source.id, source.title)}
                              title="Delete feed"
                              type="button"
                            >
                              ×
                            </button>
                          </div>
                        ))}
                  </div>
                );
              })}
            </nav>
          </>
        ) : null}

        {activeView === "sources" ? (
          <nav className="feed-list source-manager-list" aria-label="Managed sources">
            {sources.length === 0 ? (
              <section className="empty-state source-list-empty">
                <h2>No sources</h2>
                <p>Add a source to start building the local reader.</p>
              </section>
            ) : (
              sources.map((source) => (
                <div className="feed-row" key={source.id}>
                  <button
                    className={`feed-item source-manager-item ${
                      selectedManagerSource?.id === source.id ? "active" : ""
                    }`}
                    onClick={() => setSelectedManagerSourceId(source.id)}
                    type="button"
                  >
                    <span className="feed-main">
                      <span
                        className={`status-dot ${
                          source.lastError ? "error" : source.lastFetchedAt ? "healthy" : "muted"
                        }`}
                      />
                      <span className="source-list-copy">
                        <span className="feed-name">{source.title}</span>
                        <span>{source.kind.toUpperCase()} · {source.articleCount} articles</span>
                      </span>
                    </span>
                    <small>{source.unreadCount}</small>
                  </button>
                  <button
                    aria-label={`Delete ${source.title}`}
                    className="feed-delete"
                    disabled={isBusy}
                    onClick={() => requestDeleteSource(source.id, source.title)}
                    title="Delete feed"
                    type="button"
                  >
                    ×
                  </button>
                </div>
              ))
            )}
          </nav>
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
          <SourceListViewControl
            activeChoice={effectiveListView}
            installedPlugins={installedSourceListPlugins}
            onChange={handleSelectListView}
          />
          <div className="status-line">{status}</div>
        </div>

        <div
          className={`story-list ${effectiveListView === "card" ? "card" : "list"}`}
          data-source-list-plugin={effectiveListView}
        >
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
                {effectiveListView === "card" || effectiveListView === "image-board" ? (
                  <div
                    className="story-thumb"
                    style={
                      article.imageUrl ? { backgroundImage: `url(${article.imageUrl})` } : undefined
                    }
                  />
                ) : null}
                {effectiveListView === "social-stream" ? (
                  <span className="story-avatar" aria-hidden="true">
                    {article.sourceTitle.charAt(0).toUpperCase()}
                  </span>
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
                <ArticleCustomFields article={article} />
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
                    {(["rss", "rsshub", "xpath"] as const).map((mode) => (
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
                      aria-label={
                        sourceInputMode === "rss"
                          ? "Feed URL"
                          : sourceInputMode === "rsshub"
                            ? "RSSHub route"
                            : "Page URL"
                      }
                      disabled={isBusy}
                      onChange={(event) => setFeedUrl(event.currentTarget.value)}
                      placeholder={
                        sourceInputMode === "rss"
                          ? "https://example.com/feed.xml"
                          : sourceInputMode === "rsshub"
                            ? "/github/trending/daily/javascript"
                          : "https://example.com/articles"
                      }
                      value={feedUrl}
                    />
                    {sourceInputMode === "rsshub" ? (
                      <RssHubSourceForm
                        instanceId={rssHubInstanceId}
                        instances={rssHubSettings.instances}
                        isBusy={isBusy}
                        onCheckInstance={(baseUrl) => void handleCheckRssHubInstance(baseUrl)}
                        onInstanceChange={setRssHubInstanceId}
                        onTitleChange={setRssHubTitle}
                        status={rssHubStatus}
                        title={rssHubTitle}
                      />
                    ) : sourceInputMode === "xpath" ? (
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

          {selectedManagerSource ? (
            <SourceDetailPanel
              editXPathPreview={editXPathPreview}
              editXPathSelectors={editXPathSelectors}
              editXPathStatus={editXPathStatus}
              editingXPath={editingXPathSourceId === selectedManagerSource.id}
              isBusy={isBusy}
              onCancelXPathEdit={() => {
                setEditingXPathSourceId(null);
                setEditXPathPreview(null);
                setEditXPathStatus(null);
              }}
              onDelete={(id, title) => requestDeleteSource(id, title)}
              onOpenInReader={(sourceId) => {
                setActiveView("reader");
                void handleSelectSource(sourceId);
              }}
              onPreviewXPath={() => void handlePreviewXPathEdit(selectedManagerSource)}
              onRefresh={(sourceId) => void handleRefreshSource(sourceId)}
              onRename={(id, title) => void handleRenameSourceId(id, title)}
              onSaveXPath={() => void handleSaveXPathEdit(selectedManagerSource)}
              onSetCategory={(id, category) => void handleSetCategory(id, category)}
              onSetRefreshInterval={(id, seconds) =>
                void handleSetSourceRefreshInterval(id, seconds)
              }
              onSetRssHubInstance={(id, instanceId) =>
                void handleSetSourceRssHubInstance(id, instanceId)
              }
              onStartXPathEdit={() => handleStartEditXPathSource(selectedManagerSource)}
              onXPathSelectorsChange={setEditXPathSelectors}
              rssHubSettings={rssHubSettings}
              source={selectedManagerSource}
            />
          ) : (
            <section className="empty-state">
              <h2>No sources</h2>
              <p>Add a source to start building the local reader.</p>
            </section>
          )}
        </section>
      ) : null}

      {activeView === "hub" ? (
        <section className="page-view" aria-label="Hub">
          <header className="page-header hub-page-header">
            <div>
              <p className="eyebrow">Hub</p>
              <h1>Plugin Marketplace</h1>
            </div>
            <button
              className="secondary-action"
              onClick={() => void loadXPathPluginPacks(true)}
              title="Refresh plugin registry"
            >
              Refresh
            </button>
          </header>

          <section className="plugin-market-tools page-panel" aria-label="Plugin market management">
            <div className="panel-heading">
              <span>Markets</span>
              <span>{pluginMarkets.length} configured</span>
            </div>
            <div className="plugin-market-list">
              {pluginMarkets.map((market) => (
                <div className="plugin-market-row" key={market.id}>
                  <div>
                    <strong>{market.name}</strong>
                    <span>{market.repository}</span>
                  </div>
                  <span>{market.builtin ? "Built-in" : market.branch}</span>
                </div>
              ))}
            </div>
            <div className="plugin-market-actions">
              <input
                aria-label="GitHub market repository"
                onChange={(event) => setPluginMarketRepository(event.currentTarget.value)}
                placeholder="owner/repo or https://github.com/owner/repo"
                value={pluginMarketRepository}
              />
              <input
                aria-label="Market display name"
                onChange={(event) => setPluginMarketName(event.currentTarget.value)}
                placeholder="Optional market name"
                value={pluginMarketName}
              />
              <button
                disabled={isBusy || !pluginMarketRepository.trim()}
                onClick={() => void handleAddPluginMarket()}
                type="button"
              >
                Add market
              </button>
            </div>
            <div className="plugin-direct-install">
              <input
                aria-label="Plugin install URL"
                onChange={(event) => setPluginInstallUrl(event.currentTarget.value)}
                placeholder="https://.../plugin.json or xpath-rule-pack.json"
                value={pluginInstallUrl}
              />
              <button
                disabled={isBusy || !pluginInstallUrl.trim()}
                onClick={() => void handleInstallPluginUrl()}
                type="button"
              >
                Install link
              </button>
              <button disabled={isBusy} onClick={() => void handleCreatePluginMarketTemplate()} type="button">
                Create template market
              </button>
            </div>
            {pluginTemplateStatus ? <p className="hub-registry-status">{pluginTemplateStatus}</p> : null}
          </section>

          <div className="hub-search-bar">
            <input
              aria-label="Search plugins"
              className="hub-search-input"
              onChange={(e) => setHubSearchQuery(e.currentTarget.value)}
              placeholder="Search plugins by name, description, or capability..."
              type="search"
              value={hubSearchQuery}
            />
          </div>

          <nav className="hub-categories" aria-label="Install status">
            {(["all", "installed", "not-installed", "needs-update"] as const).map((filter) => (
              <button
                className={`hub-category-chip ${hubInstallFilter === filter ? "active" : ""}`}
                key={filter}
                onClick={() => setHubInstallFilter(filter)}
              >
                {filter === "all" ? "All" : filter === "installed" ? "Installed" : filter === "not-installed" ? "Not Installed" : "Needs Update"}
              </button>
            ))}
          </nav>

          <nav className="hub-categories" aria-label="Plugin type">
            {hubGroups.map((group) => (
              <button
                className={`hub-category-chip ${hubGroup === group.id ? "active" : ""}`}
                key={group.id}
                onClick={() => {
                  setHubGroup(group.id);
                  setHubCategory("all");
                }}
              >
                {group.label}
              </button>
            ))}
          </nav>
          {hubSubCategories.length > 0 && (
            <nav className="hub-subcategories" aria-label="Plugin subcategory">
              <button
                className={`hub-category-chip ${hubCategory === "all" ? "active" : ""}`}
                onClick={() => setHubCategory("all")}
              >
                All
              </button>
              {hubSubCategories.map((sub) => (
                <button
                  className={`hub-category-chip ${hubCategory === sub ? "active" : ""}`}
                  key={sub}
                  onClick={() => setHubCategory(sub)}
                >
                  {sub}
                </button>
              ))}
            </nav>
          )}

          <div className="hub-stats" aria-label="Plugin statistics">
            <span>{xpathRulePacks.length} plugins available</span>
            <span className="hub-stats-sep">·</span>
            <span>{xpathRulePacks.filter((pack) => pack.installed).length} installed</span>
            <span className="hub-stats-sep">·</span>
            <span>{officialPacks.length} official</span>
            {communityPacks.length > 0 && (
              <>
                <span className="hub-stats-sep">·</span>
                <span>{communityPacks.length} community</span>
              </>
            )}
          </div>
          {hubRegistryStatus ? <p className="hub-registry-status">{hubRegistryStatus}</p> : null}

          {officialPacks.length > 0 && (
            <section className="hub-section" aria-label="Featured plugins">
              <h2 className="hub-section-title">Featured</h2>
              <div className="hub-featured-grid">
                {officialPacks.slice(0, 3).map((pack) => (
                  <article className="hub-card hub-card-featured" key={marketplacePackKey(pack)}>
                    <HubCardIcon className="hub-card-icon" pack={pack} />
                    <div className="hub-card-body">
                      <div className="hub-card-header">
                        <span className="hub-card-name">{pack.name}</span>
                        <span className="hub-card-version">v{pack.version}</span>
                        <span className="hub-card-market">{pluginMarketLabel(pack)}</span>
                        <span className={`hub-card-trust hub-trust-${pack.trust.includes("bundled") ? "official" : pack.trust}`}>
                          {pack.trust.includes("bundled") ? "official" : pack.trust}
                        </span>
                      </div>
                      <p className="hub-card-desc">{pack.description}</p>
                      <PluginAuthorPanel pack={pack} />
                      <div className="hub-card-meta">
                        <span>{pluginKindLabel(pack)}</span>
                        <span>{pluginMetaLabel(pack)}</span>
                      </div>
                      <div className="hub-card-tags">
                        {pack.capabilities.map((cap) => (
                          <span className="hub-tag" key={cap}>{cap}</span>
                        ))}
                      </div>
                      <button
                        className="hub-add-btn primary-action"
                        onClick={() =>
                          isViewPluginPack(pack)
                            ? isViewPluginInstalled(pack)
                              ? handleUninstallViewPlugin(pack)
                              : handleInstallViewPlugin(pack)
                            : pack.installed
                              ? openPluginDialog(pack)
                              : void handleInstallPlugin(pack)
                        }
                        type="button"
                      >
                        {pluginActionLabel(pack)}
                      </button>
                      {pluginHasUpdate(pack) ? (
                        <button
                          className="hub-update-btn"
                          disabled={isBusy}
                          onClick={() => void handleUpdatePlugin(pack)}
                          type="button"
                        >
                          Update
                        </button>
                      ) : null}
                    </div>
                  </article>
                ))}
              </div>
            </section>
          )}

          <section className="hub-section" aria-label="All plugins">
            <h2 className="hub-section-title">
              {hubCategory !== "all"
                ? hubCategory
                : hubGroup === "sources"
                  ? "Source Plugins"
                  : hubGroup === "appearance"
                    ? "Appearance Plugins"
                    : "All Plugins"}
            </h2>
            {filteredPacks.length === 0 ? (
              <section className="empty-state">
                <h2>No plugins found</h2>
                <p>
                  {hubSearchQuery
                    ? `No plugins match "${hubSearchQuery}".`
                    : "No plugins available in this category."}
                </p>
              </section>
            ) : (
              <div className="hub-grid">
                {filteredPacks.map((pack) => (
                  <article className="hub-card" key={marketplacePackKey(pack)}>
                    <HubCardIcon className="hub-card-icon hub-card-icon-sm" pack={pack} />
                    <div className="hub-card-body">
                      <div className="hub-card-header">
                        <span className="hub-card-name">{pack.name}</span>
                        <span className="hub-card-version">v{pack.version}</span>
                        <span className="hub-card-market">{pluginMarketLabel(pack)}</span>
                        <span className={`hub-card-trust hub-trust-${pack.trust.includes("bundled") ? "official" : pack.trust}`}>
                          {pack.trust.includes("bundled") ? "official" : pack.trust}
                        </span>
                      </div>
                      <p className="hub-card-desc">{pack.description}</p>
                      <PluginAuthorPanel pack={pack} />
                      <div className="hub-card-meta">
                        <span>{pluginKindLabel(pack)}</span>
                        <span>{pluginMetaLabel(pack)}</span>
                      </div>
                      <div className="hub-card-tags">
                        {pack.capabilities.map((cap) => (
                          <span className="hub-tag" key={cap}>{cap}</span>
                        ))}
                      </div>
                      <button
                        className="hub-add-btn"
                        onClick={() =>
                          isViewPluginPack(pack)
                            ? isViewPluginInstalled(pack)
                              ? handleUninstallViewPlugin(pack)
                              : handleInstallViewPlugin(pack)
                            : pack.installed
                              ? openPluginDialog(pack)
                              : void handleInstallPlugin(pack)
                        }
                        type="button"
                      >
                        {pluginActionLabel(pack)}
                      </button>
                      {pluginHasUpdate(pack) ? (
                        <button
                          className="hub-update-btn"
                          disabled={isBusy}
                          onClick={() => void handleUpdatePlugin(pack)}
                          type="button"
                        >
                          Update
                        </button>
                      ) : null}
                      {pack.installed && !pack.trust.includes("bundled") && !isViewPluginPack(pack) ? (
                        <button
                          className="hub-remove-btn"
                          onClick={() => void handleUninstallPlugin(pack)}
                          type="button"
                        >
                          Uninstall
                        </button>
                      ) : null}
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>
        </section>
      ) : null}

      {showPluginDialog ? (
        <div className="dialog-overlay" onClick={closePluginDialog}>
          <section
            className="dialog-panel plugin-dialog"
            aria-label="Add source from plugin"
            onClick={(e) => e.stopPropagation()}
          >
            <header className="dialog-header">
              <h2>Add Source: {showPluginDialog.name}</h2>
              <button
                aria-label="Close dialog"
                className="dialog-close"
                onClick={closePluginDialog}
              >
                &times;
              </button>
            </header>

            <div className="dialog-body">
              {pluginParameters(showPluginDialog)?.params && pluginParameters(showPluginDialog)!.params!.length > 0 ? (
                <>
                  {pluginParameters(showPluginDialog)!.params!.map((param) => {
                    const value = dialogParamValues[param.key] ?? "";
                    return (
                      <label className="dialog-field" key={param.key}>
                        <span>{param.label}</span>
                        {param.type === "select" && param.options ? (
                          <select
                            aria-label={param.label}
                            disabled={isDialogBusy}
                            onChange={(e) => {
                              const next = { ...dialogParamValues, [param.key]: e.currentTarget.value };
                              setDialogParamValues(next);
                              setDialogUrl(pluginUrlFromParams(showPluginDialog, next));
                            }}
                            value={value}
                          >
                            {param.options.map((opt) => (
                              <option key={opt.value} value={opt.value}>
                                {opt.label}
                              </option>
                            ))}
                          </select>
                        ) : (
                          <input
                            aria-label={param.label}
                            disabled={isDialogBusy}
                            onChange={(e) => {
                              const next = { ...dialogParamValues, [param.key]: e.currentTarget.value };
                              setDialogParamValues(next);
                              setDialogUrl(pluginUrlFromParams(showPluginDialog, next));
                            }}
                            placeholder={param.placeholder}
                            type="text"
                            value={value}
                          />
                        )}
                      </label>
                    );
                  })}
                </>
              ) : null}

              <label className="dialog-field">
                <span>URL</span>
                <input
                  aria-label="Page URL"
                  disabled={isDialogBusy}
                  onChange={(e) => setDialogUrl(e.currentTarget.value)}
                  placeholder="https://..."
                  type="url"
                  value={dialogUrl}
                />
              </label>

              {pluginParameters(showPluginDialog)?.sections && pluginParameters(showPluginDialog)!.sections!.length > 0 ? (
                <label className="dialog-field">
                  <span>Section</span>
                  <select
                    aria-label="Forum section"
                    disabled={isDialogBusy}
                    onChange={(e) => {
                      const sec = pluginParameters(showPluginDialog)!.sections!.find(
                        (s) => s.id === e.currentTarget.value,
                      );
                      if (sec) {
                        setDialogSectionId(sec.id);
                        setDialogUrl(sec.url);
                        setDialogTitle(pluginSourceTitle(showPluginDialog, sec));
                      }
                    }}
                    value={dialogSectionId}
                  >
                    {pluginParameters(showPluginDialog)!.sections!.map((sec) => (
                      <option key={sec.id} value={sec.id}>
                        {sec.path.join(" > ")}
                      </option>
                    ))}
                  </select>
                </label>
              ) : null}

              {pluginCandidates(showPluginDialog).length > 1 ? (
                <label className="dialog-field">
                  <span>Rule</span>
                  <select
                    aria-label="Plugin rule"
                    disabled={isDialogBusy}
                    onChange={(e) => {
                      const candidateId = e.currentTarget.value;
                      const candidate = pluginCandidates(showPluginDialog).find((item) => item.id === candidateId);
                      setDialogCandidateId(candidateId);
                      setDialogMaxItems(
                        candidate?.selectors.maxItems ?? pluginParameters(showPluginDialog)?.defaults?.maxItems,
                      );
                      setDialogPreview(null);
                    }}
                    value={dialogCandidateId}
                  >
                    {pluginCandidates(showPluginDialog).map((candidate) => (
                      <option key={candidate.id} value={candidate.id}>
                        {candidate.pageType}
                      </option>
                    ))}
                  </select>
                </label>
              ) : null}

              <label className="dialog-field">
                <span>Source title</span>
                <input
                  aria-label="Source title"
                  disabled={isDialogBusy}
                  onChange={(e) => setDialogTitle(e.currentTarget.value)}
                  placeholder="Source title"
                  type="text"
                  value={dialogTitle}
                />
              </label>

              <label className="dialog-field">
                <span>Max items per refresh</span>
                <input
                  aria-label="Max items per refresh"
                  disabled={isDialogBusy}
                  min="1"
                  onChange={(e) => setDialogMaxItems(parseOptionalPositiveInt(e.currentTarget.value))}
                  placeholder="No limit"
                  type="number"
                  value={dialogMaxItems ?? ""}
                />
              </label>

              <label className="dialog-field">
                <span>Plugin Cookie</span>
                <input
                  aria-label="Plugin cookie header"
                  disabled={isDialogBusy}
                  onChange={(e) => setDialogCookie(e.currentTarget.value)}
                  type="password"
                  value={dialogCookie}
                  placeholder={pluginCredential?.cookieSet ? "Cookie saved — type to replace" : "name=value; ... or $FEADER_NAIXI_COOKIE"}
                />
                <small className="cookie-field-hint">
                  {pluginCredential?.cookieSet
                    ? "Cookie saved (masked). Type to replace."
                    : "Empty = use plugin cookie"}
                </small>
                <div className="cookie-field-actions">
                  <button
                    type="button"
                    className="hub-cookie-save"
                    disabled={isDialogBusy || !showPluginDialog}
                    onClick={async () => {
                      if (!showPluginDialog) return;
                      try {
                        const updated = await invoke<PluginCredential>("set_plugin_credential", {
                          pluginId: showPluginDialog.id,
                          cookie: dialogCookie,
                        });
                        setPluginCredential(updated);
                        setStatus("Cookie saved");
                      } catch (error) {
                        setStatus(String(error));
                      }
                    }}
                  >
                    Save cookie
                  </button>
                  {pluginAuth(showPluginDialog) ? (
                    <button
                      type="button"
                      className="hub-cookie-check"
                      disabled={isDialogBusy}
                      onClick={async () => {
                        if (!showPluginDialog) return;
                        try {
                          const result = await invoke<CredentialCheck>("check_plugin_credential", {
                            pluginId: showPluginDialog.id,
                            checkUrl: pluginAuth(showPluginDialog)!.checkUrl,
                            loggedInXpath: pluginAuth(showPluginDialog)!.loggedInXpath,
                          });
                          setCredentialCheck(result);
                        } catch (error) {
                          setCredentialCheck({ ok: false, message: String(error), checkedAt: new Date().toISOString() });
                        }
                      }}
                    >
                      Check cookie
                    </button>
                  ) : null}
                </div>
                {credentialCheck ? (
                  <small className={credentialCheck.ok ? "cookie-status ok" : "cookie-status bad"}>
                    {credentialCheck.message}
                  </small>
                ) : null}
              </label>

              {dialogPreview ? (
                <div className="dialog-preview-summary">
                  Preview: {dialogPreview.articles.length} articles found
                  {dialogPreview.nextPageUrl ? " · next page available" : ""}
                </div>
              ) : null}

              {dialogStatus ? (
                <div className="dialog-status">{dialogStatus}</div>
              ) : null}
            </div>

            <footer className="dialog-footer">
              <button
                className="secondary-action"
                disabled={isDialogBusy}
                onClick={handleDialogPreview}
              >
                Preview
              </button>
              <button
                className="primary-action"
                disabled={isDialogBusy}
                onClick={handleDialogAddSource}
              >
                Add Source
              </button>
            </footer>
          </section>
        </div>
      ) : null}

      {pendingDelete ? (
        <div className="dialog-overlay" onClick={() => setPendingDelete(null)}>
          <section
            aria-label="Confirm delete feed"
            className="dialog-panel confirm-dialog"
            onClick={(e) => e.stopPropagation()}
          >
            <header className="dialog-header">
              <h2>Delete feed</h2>
            </header>
            <div className="dialog-body">
              <p>
                Delete &ldquo;{pendingDelete.title}&rdquo; and all of its articles? This cannot be
                undone.
              </p>
            </div>
            <footer className="dialog-footer">
              <button
                className="secondary-action"
                disabled={isBusy}
                onClick={() => setPendingDelete(null)}
                type="button"
              >
                Cancel
              </button>
              <button
                className="danger-action"
                disabled={isBusy}
                onClick={() => {
                  const target = pendingDelete;
                  setPendingDelete(null);
                  void handleDeleteSourceId(target.id, target.title);
                }}
                type="button"
              >
                Delete feed
              </button>
            </footer>
          </section>
        </div>
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
                <span>{themeStatusLabel(themeMode)}</span>
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

            <RssHubSettingsCard
              disabled={isBusy}
              onAddInstance={(name, baseUrl) => void handleAddRssHubInstance(name, baseUrl)}
              onCheckInstance={(baseUrl) => void handleCheckRssHubInstance(baseUrl)}
              onGlobalInstanceChange={(instanceId) => void handleSetRssHubGlobalInstance(instanceId)}
              settings={rssHubSettings}
              status={rssHubStatus}
            />

            {installedRuntimePlugins.map((plugin) => (
              <RuntimePluginSettingsCard
                disabled={isBusy}
                key={plugin.id}
                plugin={plugin}
              />
            ))}

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

            <PluginSwitchboard
              appUiThemeByMode={appUiThemeByMode}
              appUiPlugins={appUiPluginOptions}
              detailViewPlugin={detailViewPlugin}
              installedViewPlugins={installedViewPlugins}
              onActivateDetailView={setDetailViewPlugin}
              onAssignTheme={handleAssignTheme}
            />

            <article className="settings-card">
              <div className="panel-heading">
                <span>Auto Refresh</span>
                <span>
                  {autoRefreshConfig?.enabled
                    ? refreshTick?.refreshing
                      ? "Refreshing…"
                      : refreshTick?.nextRefreshAt
                        ? `Next: ${formatCountdown(refreshTick.nextRefreshAt)}`
                        : `Every ${formatInterval(autoRefreshConfig?.globalIntervalSeconds ?? 1800)}`
                    : "Paused"}
                </span>
              </div>
              <label className="preference-strip">
                <span>Enable background refresh</span>
                <input
                  type="checkbox"
                  checked={autoRefreshConfig?.enabled ?? true}
                  onChange={(e) => void handleToggleAutoRefresh(e.target.checked)}
                />
              </label>
              <div style={{ marginTop: "0.75rem" }}>
                <span className="preference-label">Global interval</span>
                <div className="interval-presets" style={{ display: "flex", gap: "0.35rem", flexWrap: "wrap", marginTop: "0.35rem" }}>
                  {REFRESH_INTERVAL_PRESETS.map((preset) => (
                    <button
                      key={preset.seconds}
                      className={`chip ${
                        (autoRefreshConfig?.globalIntervalSeconds ?? 1800) === preset.seconds
                          ? "active"
                          : ""
                      }`}
                      onClick={() => void handleSetGlobalRefreshInterval(preset.seconds)}
                      type="button"
                    >
                      {preset.label}
                    </button>
                  ))}
                </div>
              </div>
              {autoRefreshConfig?.pluginOverrides && autoRefreshConfig.pluginOverrides.length > 0 ? (
                <div style={{ marginTop: "0.75rem" }}>
                  <span className="preference-label">Plugin overrides</span>
                  <dl style={{ marginTop: "0.35rem" }}>
                    {autoRefreshConfig.pluginOverrides.map((ov) => (
                      <div key={ov.pluginId} style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "0.25rem 0" }}>
                        <dt style={{ flex: 1 }}>{ov.pluginName}</dt>
                        <dd>
                          <select
                            value={ov.refreshIntervalSeconds}
                            onChange={(e) =>
                              void handleSetPluginRefreshInterval(ov.pluginId, Number(e.target.value))
                            }
                          >
                            {REFRESH_INTERVAL_PRESETS.map((p) => (
                              <option key={p.seconds} value={p.seconds}>
                                {p.label}
                              </option>
                            ))}
                          </select>
                        </dd>
                      </div>
                    ))}
                  </dl>
                </div>
              ) : null}
              {refreshTick?.refreshing ? (
                <div className="preference-strip" style={{ marginTop: "0.75rem" }}>
                  <span>
                    {refreshTick.currentSourceTitle
                      ? `Refreshing: ${refreshTick.currentSourceTitle}`
                      : "Checking sources…"}
                  </span>
                  <span>
                    {refreshTick.sourcesRefreshed}/{refreshTick.sourcesChecked}
                  </span>
                </div>
              ) : null}
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
              detailViewPlugin={detailViewPlugin}
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
              detailViewPlugin={detailViewPlugin}
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
  onToggleTheme,
}: {
  activeView: ViewMode;
  onSelectView: (view: ViewMode) => void;
  themeMode: ThemeMode;
  onToggleTheme: () => void;
}) {
  const resolvedTheme = resolveThemeMode(themeMode);

  return (
    <nav className="icon-rail" aria-label="Primary">
      <span className="rail-mark" aria-hidden="true">F</span>
      {(["reader", "sources", "hub"] as const).map((view) => (
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
        aria-label={`Switch to ${resolvedTheme === "dark" ? "light" : "dark"} theme`}
        className="rail-button"
        onClick={onToggleTheme}
        type="button"
      >
        {railIcon(resolvedTheme === "dark" ? "light-theme" : "dark-theme")}
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

function railIcon(name: ViewMode | "light-theme" | "dark-theme") {
  const paths: Record<string, string> = {
    reader: "M4 6h16M4 12h16M4 18h11",
    sources: "M4 4h16v16H4zM4 9.5h16",
    hub: "M12 3l8 4.5v9L12 21l-8-4.5v-9L12 3zM12 12l8-4.5M12 12v9M12 12L4 7.5",
    "light-theme": "M12 3v2M12 19v2M5.64 5.64l1.42 1.42M16.94 16.94l1.42 1.42M3 12h2M19 12h2M5.64 18.36l1.42-1.42M16.94 7.06l1.42-1.42M12 8a4 4 0 100 8 4 4 0 000-8z",
    "dark-theme": "M21 13.2A7.8 7.8 0 1110.8 3a6.3 6.3 0 0010.2 10.2z",
    settings: "M12 9a3 3 0 100 6 3 3 0 000-6zM12 2v3M12 19v3M2 12h3M19 12h3",
  };
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <path d={paths[name]} />
    </svg>
  );
}

function oppositeResolvedThemeMode(mode: ThemeMode): Exclude<ThemeMode, "system"> {
  return resolveThemeMode(mode) === "dark" ? "light" : "dark";
}

function ThemeControl({
  mode,
  onChange,
}: {
  mode: ThemeMode;
  onChange: (mode: ThemeMode) => void;
}) {
  const resolvedMode = resolveThemeMode(mode);
  const isFollowingSystem = mode === "system";

  return (
    <div className="theme-control">
      <button
        aria-label={`Switch to ${resolvedMode === "dark" ? "light" : "dark"} theme`}
        aria-pressed={resolvedMode === "dark"}
        className={`theme-switch ${resolvedMode === "dark" ? "dark" : "light"}`}
        onClick={() => onChange(oppositeResolvedThemeMode(mode))}
        type="button"
      >
        <span className="theme-switch-option">Light</span>
        <span className="theme-switch-option">Dark</span>
        <span className="theme-switch-thumb" aria-hidden="true">
          {resolvedMode === "dark" ? "Dark" : "Light"}
        </span>
      </button>
      <label className={`system-theme-toggle ${isFollowingSystem ? "active" : ""}`}>
        <input
          checked={isFollowingSystem}
          onChange={(event) => {
            onChange(event.target.checked ? "system" : resolvedMode);
          }}
          type="checkbox"
        />
        <span>Follow system</span>
      </label>
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
          <span>Request URL</span>
          <input
            disabled={disabled}
            onChange={(event) => setBaseUrl(event.currentTarget.value)}
            placeholder={
              provider === "anthropic"
                ? "https://api.anthropic.com"
                : "https://api.openai.com/v1"
            }
            value={baseUrl}
          />
          <small className="selector-hint">
            Enter a provider base URL or a complete API URL.
          </small>
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

function RssHubSourceForm({
  instanceId,
  instances,
  isBusy,
  onCheckInstance,
  onInstanceChange,
  onTitleChange,
  status,
  title,
}: {
  instanceId: string;
  instances: RssHubInstance[];
  isBusy: boolean;
  onCheckInstance: (baseUrl: string) => void;
  onInstanceChange: (instanceId: string) => void;
  onTitleChange: (title: string) => void;
  status: string | null;
  title: string;
}) {
  const selectedInstance = instances.find((instance) => instance.id === instanceId);
  const fallbackInstance =
    instances.find((instance) => instance.id === defaultRssHubSettings.globalInstanceId) ??
    instances[0];
  return (
    <section className="rsshub-form">
      <label>
        <span>Title</span>
        <input
          disabled={isBusy}
          onChange={(event) => onTitleChange(event.currentTarget.value)}
          placeholder="Optional, uses upstream feed title when blank"
          value={title}
        />
      </label>
      <label>
        <span>Instance override</span>
        <select
          disabled={isBusy}
          onChange={(event) => onInstanceChange(event.currentTarget.value)}
          value={instanceId}
        >
          <option value="">Inherit global instance</option>
          {instances.map((instance) => (
            <option key={instance.id} value={instance.id}>
              {instance.name} · {instance.baseUrl}
            </option>
          ))}
        </select>
      </label>
      <div className="adapter-summary rsshub-instance-summary" aria-label="RSSHub instance status">
        <span>RSSHub route</span>
        <span>{selectedInstance ? selectedInstance.name : "Global default"}</span>
        <button
          disabled={isBusy || !fallbackInstance}
          onClick={() => onCheckInstance(selectedInstance?.baseUrl ?? fallbackInstance.baseUrl)}
          type="button"
        >
          Check availability
        </button>
      </div>
      {status ? <p className="xpath-status">{status}</p> : null}
    </section>
  );
}

function RssHubSettingsCard({
  disabled,
  onAddInstance,
  onCheckInstance,
  onGlobalInstanceChange,
  settings,
  status,
}: {
  disabled: boolean;
  onAddInstance: (name: string, baseUrl: string) => void;
  onCheckInstance: (baseUrl: string) => void;
  onGlobalInstanceChange: (instanceId: string) => void;
  settings: RssHubSettings;
  status: string | null;
}) {
  const [name, setName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const globalInstance =
    settings.instances.find((instance) => instance.id === settings.globalInstanceId) ??
    settings.instances[0];

  return (
    <article className="settings-card rsshub-settings-card">
      <div className="panel-heading">
        <span>RSSHub</span>
        <span>{globalInstance?.name ?? "Not configured"}</span>
      </div>
      <label className="selector-input">
        <span>Global instance</span>
        <select
          disabled={disabled}
          onChange={(event) => onGlobalInstanceChange(event.currentTarget.value)}
          value={settings.globalInstanceId}
        >
          {settings.instances.map((instance) => (
            <option key={instance.id} value={instance.id}>
              {instance.name} · {instance.baseUrl}
            </option>
          ))}
        </select>
      </label>
      <div className="rsshub-instance-list">
        {settings.instances.map((instance) => (
          <div className="rsshub-instance-row" key={instance.id}>
            <div>
              <strong>{instance.name}</strong>
              <span>{instance.baseUrl}</span>
            </div>
            <button disabled={disabled} onClick={() => onCheckInstance(instance.baseUrl)} type="button">
              Check
            </button>
          </div>
        ))}
      </div>
      <div className="rsshub-custom-instance">
        <input
          disabled={disabled}
          onChange={(event) => setName(event.currentTarget.value)}
          placeholder="Custom name"
          value={name}
        />
        <input
          disabled={disabled}
          onChange={(event) => setBaseUrl(event.currentTarget.value)}
          placeholder="https://rsshub.example.com"
          value={baseUrl}
        />
        <button
          disabled={disabled || !baseUrl.trim()}
          onClick={() => {
            onAddInstance(name, baseUrl);
            setName("");
            setBaseUrl("");
          }}
          type="button"
        >
          Add
        </button>
      </div>
      {status ? <p className="xpath-status">{status}</p> : null}
    </article>
  );
}

function RuntimePluginSettingsCard({
  disabled,
  plugin,
}: {
  disabled: boolean;
  plugin: MarketplacePluginPack;
}) {
  const page = plugin.runtime?.settingsPage;
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [importText, setImportText] = useState("");
  const [exportText, setExportText] = useState("");
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<Record<string, unknown>>("get_plugin_config", { pluginId: plugin.id })
      .then((next) => {
        if (!cancelled) setValues(next);
      })
      .catch((error) => {
        if (!cancelled) setStatus(String(error));
      });
    return () => {
      cancelled = true;
    };
  }, [plugin.id]);

  if (!page) return null;

  function setField(key: string, value: unknown): void {
    setValues((current) => ({ ...current, [key]: value }));
  }

  async function save(): Promise<void> {
    setStatus("Saving...");
    try {
      const saved = await invoke<Record<string, unknown>>("set_plugin_config", {
        request: { pluginId: plugin.id, values },
      });
      setValues(saved);
      setStatus("Settings saved");
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function exportConfig(): Promise<void> {
    try {
      const json = await invoke<string>("export_plugin_config", { pluginId: plugin.id });
      setExportText(json);
      setStatus("Export ready");
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function importConfig(): Promise<void> {
    try {
      const imported = await invoke<Record<string, unknown>>("import_plugin_config", {
        request: { pluginId: plugin.id, json: importText },
      });
      setValues(imported);
      setStatus("Config imported");
    } catch (error) {
      setStatus(String(error));
    }
  }

  return (
    <article className="settings-card">
      <div className="panel-heading">
        <span>{page.title}</span>
        <span>{plugin.runtime?.runtime.engine ?? "Runtime"}</span>
      </div>
      {page.sections.map((section) => (
        <section className="plugin-settings-section" key={section.id}>
          <div className="panel-subheading">
            <span>{section.title}</span>
            {section.description ? <small>{section.description}</small> : null}
          </div>
          {section.fields.map((field) => (
            <label className="selector-input" key={field.key}>
              <span>{field.label}</span>
              {field.type === "boolean" ? (
                <input
                  checked={Boolean(values[field.key] ?? field.default ?? false)}
                  disabled={disabled}
                  onChange={(event) => setField(field.key, event.currentTarget.checked)}
                  type="checkbox"
                />
              ) : field.type === "select" && field.options ? (
                <select
                  disabled={disabled}
                  onChange={(event) => setField(field.key, event.currentTarget.value)}
                  value={String(values[field.key] ?? field.default ?? "")}
                >
                  {field.options.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              ) : field.type === "secret" ? (
                <input
                  disabled
                  placeholder="Secrets are stored through plugin credentials and are not exported."
                  type="password"
                />
              ) : (
                <input
                  disabled={disabled}
                  onChange={(event) =>
                    setField(
                      field.key,
                      field.type === "number"
                        ? Number(event.currentTarget.value)
                        : event.currentTarget.value,
                    )
                  }
                  placeholder={field.placeholder ?? undefined}
                  type={field.type === "number" ? "number" : "text"}
                  value={String(values[field.key] ?? field.default ?? "")}
                />
              )}
              {field.help ? <small className="selector-hint">{field.help}</small> : null}
            </label>
          ))}
        </section>
      ))}
      <div className="ai-actions">
        <button className="primary-action" disabled={disabled} onClick={() => void save()} type="button">
          Save plugin settings
        </button>
        <button className="secondary-action" disabled={disabled} onClick={() => void exportConfig()} type="button">
          Export config
        </button>
      </div>
      {exportText ? (
        <textarea
          className="plugin-config-textarea"
          readOnly
          value={exportText}
        />
      ) : null}
      <label className="selector-input">
        <span>Import config</span>
        <textarea
          className="plugin-config-textarea"
          disabled={disabled}
          onChange={(event) => setImportText(event.currentTarget.value)}
          placeholder='{"schemaVersion":"feader-plugin-config/v1",...}'
          value={importText}
        />
      </label>
      <button className="secondary-action" disabled={disabled || !importText.trim()} onClick={() => void importConfig()} type="button">
        Import config
      </button>
      {status ? <p className="xpath-status">{status}</p> : null}
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

function SourceListViewControl({
  activeChoice,
  installedPlugins,
  onChange,
}: {
  activeChoice: string;
  installedPlugins: ViewPluginDefinition<SourceListPluginId>[];
  onChange: (choice: string) => void;
}) {
  const options: { id: string; label: string }[] = [
    { id: "list", label: "List" },
    { id: "card", label: "Card" },
    ...installedPlugins.map((plugin) => ({ id: plugin.id, label: plugin.name })),
  ];
  return (
    <div className="entry-layout-control source-list-view-control" role="group" aria-label="Source list view">
      {options.map((option) => (
        <button
          aria-pressed={activeChoice === option.id}
          className={activeChoice === option.id ? "active" : ""}
          key={option.id}
          onClick={() => onChange(option.id)}
          type="button"
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function SourceDetailPanel({
  editXPathPreview,
  editXPathSelectors,
  editXPathStatus,
  editingXPath,
  isBusy,
  onCancelXPathEdit,
  onDelete,
  onOpenInReader,
  onPreviewXPath,
  onRefresh,
  onRename,
  onSaveXPath,
  onSetCategory,
  onSetRefreshInterval,
  onSetRssHubInstance,
  onStartXPathEdit,
  onXPathSelectorsChange,
  rssHubSettings,
  source,
}: {
  editXPathPreview: XPathPreview | null;
  editXPathSelectors: XPathSelectors;
  editXPathStatus: string | null;
  editingXPath: boolean;
  isBusy: boolean;
  onCancelXPathEdit: () => void;
  onDelete: (sourceId: number, title: string) => void;
  onOpenInReader: (sourceId: number) => void;
  onPreviewXPath: () => void;
  onRefresh: (sourceId: number) => void;
  onRename: (sourceId: number, title: string) => void;
  onSaveXPath: () => void;
  onSetCategory: (sourceId: number, category: string) => void;
  onSetRefreshInterval: (sourceId: number, seconds: number | null) => void;
  onSetRssHubInstance: (sourceId: number, instanceId: string | null) => void;
  onStartXPathEdit: () => void;
  onXPathSelectorsChange: (selectors: XPathSelectors) => void;
  rssHubSettings: RssHubSettings;
  source: Source;
}) {
  const selectors = source.kind === "xpath" ? readXPathSelectorsFromSource(source) : null;
  const rssHubConfig = source.kind === "rsshub" ? readRssHubConfigFromSource(source) : null;

  return (
    <article className="source-detail page-panel">
      <div className="source-detail-header">
        <div>
          <p className="eyebrow">{source.kind.toUpperCase()} · {sourceHealth(source)}</p>
          <h2>{source.title}</h2>
        </div>
        <div className="story-actions source-detail-actions">
          <button disabled={isBusy} onClick={() => onOpenInReader(source.id)} type="button">
            Open in reader
          </button>
          <button disabled={isBusy} onClick={() => onRefresh(source.id)} type="button">
            Refresh
          </button>
        </div>
      </div>

      <SourceHealthStrip source={source} />

      <section className="source-detail-grid">
        <div className="source-detail-section">
          <div className="panel-heading">
            <span>Identity</span>
            <span>Manage</span>
          </div>
          <CategoryPicker disabled={isBusy} onSubmit={onSetCategory} source={source} />
          <SourceCardManage
            disabled={isBusy}
            onDelete={onDelete}
            onRename={onRename}
            source={source}
          />
        </div>

        <div className="source-detail-section">
          <div className="panel-heading">
            <span>Details</span>
            <span>{sourceDiagnostic(source)}</span>
          </div>
          <dl>
            <dt>Kind</dt>
            <dd>{source.kind}</dd>
            <dt>URL</dt>
            <dd>{source.url}</dd>
            <dt>Category</dt>
            <dd>{source.category?.trim() || uncategorizedLabel}</dd>
            <dt>Enabled</dt>
            <dd>{source.enabled ? "Yes" : "No"}</dd>
            <dt>Unread</dt>
            <dd>{source.unreadCount}</dd>
            <dt>Articles</dt>
            <dd>{source.articleCount}</dd>
            <dt>Last refresh</dt>
            <dd>{formatDate(source.lastFetchedAt)}</dd>
            <dt>Refresh interval</dt>
            <dd>
              <select
                value={source.refreshIntervalSeconds ?? ""}
                onChange={(e) => {
                  const val = e.target.value;
                  onSetRefreshInterval(source.id, val ? Number(val) : null);
                }}
                style={{ fontSize: "0.85rem" }}
              >
                <option value="">Inherit (global)</option>
                {REFRESH_INTERVAL_PRESETS.map((p) => (
                  <option key={p.seconds} value={p.seconds}>
                    {p.label}
                  </option>
                ))}
              </select>
            </dd>
            {source.kind === "rsshub" ? (
              <>
                <dt>RSSHub route</dt>
                <dd>{rssHubConfig?.route ?? source.url}</dd>
                <dt>RSSHub instance</dt>
                <dd>
                  <select
                    value={rssHubConfig?.instanceId ?? ""}
                    onChange={(event) =>
                      onSetRssHubInstance(source.id, event.currentTarget.value || null)
                    }
                    style={{ fontSize: "0.85rem" }}
                  >
                    <option value="">Inherit global ({rssHubGlobalName(rssHubSettings)})</option>
                    {rssHubSettings.instances.map((instance) => (
                      <option key={instance.id} value={instance.id}>
                        {instance.name}
                      </option>
                    ))}
                  </select>
                </dd>
              </>
            ) : null}
          </dl>
          {source.lastError ? <p className="error-text">{source.lastError}</p> : null}
        </div>
      </section>

      {source.kind === "xpath" ? <SourcePluginSummary plugin={selectors?.plugin} /> : null}

      {source.kind === "xpath" ? (
        <section className="xpath-editor-panel">
          <div className="panel-heading">
            <span>XPath selectors</span>
            <span>{editingXPath ? "Editing" : "Static DOM"}</span>
          </div>
          {editingXPath ? (
            <>
              <XPathSourceForm
                aiAvailable={false}
                isBusy={isBusy}
                onPreview={onPreviewXPath}
                onSelectorsChange={onXPathSelectorsChange}
                onSuggest={() => undefined}
                onTitleChange={() => undefined}
                preview={editXPathPreview}
                selectors={editXPathSelectors}
                showTitle={false}
                status={editXPathStatus}
                title={source.title}
              />
              <div className="story-actions xpath-edit-actions">
                <button disabled={isBusy} onClick={onCancelXPathEdit} type="button">
                  Cancel
                </button>
                <button className="primary-action" disabled={isBusy} onClick={onSaveXPath} type="button">
                  Save selectors
                </button>
              </div>
            </>
          ) : (
            <>
              <XPathSelectorSummary selectors={selectors} />
              <button disabled={isBusy} onClick={onStartXPathEdit} type="button">
                Edit XPath selectors
              </button>
            </>
          )}
        </section>
      ) : null}
    </article>
  );
}

function XPathSelectorSummary({ selectors }: { selectors: XPathSelectors | null }) {
  if (!selectors) {
    return <p className="error-text">Selector config could not be parsed.</p>;
  }
  const rows: [string, string | undefined][] = [
    ["Items", selectors.items],
    ["Title", selectors.title],
    ["URL", selectors.url],
    ["Summary", selectors.summary],
    ["Date", selectors.publishedAt],
    ["Author", selectors.author],
    ["Cookie", cookieSummary(selectors.cookie)],
    ["Content", selectors.content],
    ["Detail content", selectors.detailContent],
    ["Image", selectors.image],
    ["Next page", selectors.nextPage],
    ["Max items", selectors.maxItems ? String(selectors.maxItems) : undefined],
    ["Cleanup rules", selectors.contentCleanup?.length ? `${selectors.contentCleanup.length} configured` : undefined],
    ["Custom fields", customFieldSummary(selectors.customFields)],
  ];
  return (
    <dl className="xpath-selector-summary">
      {rows.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value?.trim() || "Unset"}</dd>
        </div>
      ))}
    </dl>
  );
}

function HubCardIcon({ pack, className }: { pack: PluginPack; className: string }) {
  if (pack.logo) {
    return (
      <div className={className}>
        <img alt="" className="hub-card-logo" src={pack.logo} />
      </div>
    );
  }
  return <div className={className}>{pack.name.charAt(0).toUpperCase()}</div>;
}

function SourcePluginSummary({ plugin }: { plugin?: XPathSourcePluginInfo }) {
  if (!plugin) return null;
  return (
    <section className="source-detail-section source-plugin-section">
      <div className="panel-heading">
        <span>Plugin</span>
        <span>{plugin.trust}</span>
      </div>
      <dl>
        <dt>Name</dt>
        <dd>{plugin.name}</dd>
        <dt>Version</dt>
        <dd>v{plugin.version}</dd>
        <dt>Rule</dt>
        <dd>{plugin.pageType}</dd>
        <dt>Registry</dt>
        <dd>{plugin.registry}</dd>
      </dl>
      <PluginAuthorDetails authors={plugin.authors} />
      <div className="hub-card-tags">
        {plugin.capabilities.map((cap) => (
          <span className="hub-tag" key={cap}>{cap}</span>
        ))}
      </div>
    </section>
  );
}

function pluginSourceTitle(pack: PluginPack, section?: PluginSection): string {
  const sectionName = section?.path[section.path.length - 1]?.trim();
  if (!sectionName) {
    return pack.name;
  }
  if (pack.id === "official.naixi-forum.xpath") {
    return `奶昔论坛 · ${sectionName}`;
  }
  return `${pack.name} · ${sectionName}`;
}

function pluginSourceInfo(pack: PluginPack, candidate: XPathRuleCandidate): XPathSourcePluginInfo {
  return {
    id: pack.id,
    name: pack.name,
    version: pack.version,
    registry: pack.registry,
    trust: pack.trust,
    candidateId: candidate.id,
    pageType: candidate.pageType,
    capabilities: pack.capabilities,
    authors: pack.authors,
  };
}

function PluginAuthorPanel({ pack }: { pack: PluginPack }) {
  return <PluginAuthorDetails authors={pack.authors} />;
}

function PluginAuthorDetails({ authors }: { authors?: PluginAuthor[] }) {
  const author = authors?.[0];
  const [openPanel, setOpenPanel] = useState<"more" | "donate" | null>(null);
  if (!author) return null;
  const profileUrl = author.website || (author.githubId ? `https://github.com/${author.githubId}` : undefined);
  const githubUrl = author.githubId ? `https://github.com/${author.githubId}` : undefined;
  const hasMore = Boolean(author.email || author.website || author.githubId);
  const togglePanel = (panel: "more" | "donate") =>
    setOpenPanel((current) => (current === panel ? null : panel));
  const closePanel = () => setOpenPanel(null);
  return (
    <div className="hub-author">
      {author.avatarUrl ? <img alt="" className="hub-author-avatar" src={author.avatarUrl} /> : null}
      <div className="hub-author-main">
        <div className="hub-author-line">
          {profileUrl ? (
            <a href={profileUrl} rel="noreferrer" target="_blank">
              {author.name}
            </a>
          ) : (
            <span>{author.name}</span>
          )}
          {author.githubId ? <span>@{author.githubId}</span> : null}
        </div>
        <div className="hub-author-actions">
          {hasMore ? (
            <button
              className="hub-author-action"
              onClick={() => togglePanel("more")}
              type="button"
            >
              {openPanel === "more" ? "Hide" : "See more"}
            </button>
          ) : null}
          {author.evmAddress ? (
            <button
              className="hub-author-action"
              onClick={() => togglePanel("donate")}
              type="button"
            >
              {openPanel === "donate" ? "Hide" : "Donate"}
            </button>
          ) : null}
        </div>
      </div>
      {openPanel === "more" ? (
        <div className="hub-author-popover">
          <button
            aria-label="Close"
            className="hub-author-popover-close"
            onClick={closePanel}
            type="button"
          >
            ×
          </button>
          <dl className="hub-author-detail-list">
            {author.email ? (
              <>
                <dt>Email</dt>
                <dd>
                  <a href={`mailto:${author.email}`}>{author.email}</a>
                </dd>
              </>
            ) : null}
            {author.website ? (
              <>
                <dt>Website</dt>
                <dd>
                  <a href={author.website} rel="noreferrer" target="_blank">
                    {author.website}
                  </a>
                </dd>
              </>
            ) : null}
            {githubUrl ? (
              <>
                <dt>GitHub</dt>
                <dd>
                  <a href={githubUrl} rel="noreferrer" target="_blank">
                    @{author.githubId}
                  </a>
                </dd>
              </>
            ) : null}
          </dl>
        </div>
      ) : null}
      {openPanel === "donate" && author.evmAddress ? (
        <div className="hub-author-popover">
          <button
            aria-label="Close"
            className="hub-author-popover-close"
            onClick={closePanel}
            type="button"
          >
            ×
          </button>
          <div className="hub-donate-detail">
            <img
              alt="EVM donation QR"
              className="hub-donate-qr"
              src={qrImageUrl(evmDonateUri(author.evmAddress))}
            />
            <code className="hub-donate-address">{author.evmAddress}</code>
            <a href={evmDonateUri(author.evmAddress)} rel="noreferrer" target="_blank">
              Open in wallet
            </a>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function evmDonateUri(address: string): string {
  return `ethereum:${address}`;
}

function qrImageUrl(value: string): string {
  return `https://api.qrserver.com/v1/create-qr-code/?size=112x112&data=${encodeURIComponent(value)}`;
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

function PluginSwitchboard({
  appUiThemeByMode,
  appUiPlugins,
  detailViewPlugin,
  installedViewPlugins,
  onActivateDetailView,
  onAssignTheme,
}: {
  appUiThemeByMode: AppUiThemeByMode;
  appUiPlugins: ViewPluginDefinition<AppUiPluginId>[];
  detailViewPlugin: DetailViewPluginId | null;
  installedViewPlugins: string[];
  onActivateDetailView: (plugin: DetailViewPluginId | null) => void;
  onAssignTheme: (mode: "light" | "dark", pluginId: AppUiPluginId | null) => void;
}) {
  const installedAppUi = appUiPlugins.filter((plugin) => installedViewPlugins.includes(plugin.id));
  const installedDetail = detailViewPlugins.filter((plugin) =>
    installedViewPlugins.includes(plugin.id),
  );
  return (
    <article className="settings-card plugin-switchboard">
      <div className="panel-heading">
        <span>View plugins</span>
        <span>{installedViewPlugins.length} installed</span>
      </div>
      <p className="plugin-switchboard-copy">
        Install appearance plugins from the Plugin Hub, then assign a theme to Light and Dark and pick
        an article detail view. Source list views are chosen per source in the reading queue.
      </p>
      <ActivationSlot
        activeId={appUiThemeByMode.light}
        baseLabel="Light"
        baseHint="Built-in light appearance"
        label="Light theme"
        onChange={(id) => onAssignTheme("light", id)}
        options={installedAppUi}
      />
      <ActivationSlot
        activeId={appUiThemeByMode.dark}
        baseLabel="Dark"
        baseHint="Built-in dark appearance"
        label="Dark theme"
        onChange={(id) => onAssignTheme("dark", id)}
        options={installedAppUi}
      />
      <ActivationSlot
        activeId={detailViewPlugin}
        baseLabel="Native"
        baseHint="Feader native article detail"
        label="Detail content view"
        onChange={onActivateDetailView}
        options={installedDetail}
      />
    </article>
  );
}

function ActivationSlot<T extends string>({
  activeId,
  baseHint,
  baseLabel,
  label,
  onChange,
  options,
}: {
  activeId: T | null;
  baseHint: string;
  baseLabel: string;
  label: string;
  onChange: (pluginId: T | null) => void;
  options: ViewPluginDefinition<T>[];
}) {
  return (
    <section className="plugin-slot" aria-label={label}>
      <div className="plugin-slot-header">
        <div>
          <strong>{label}</strong>
          <span>{activeId ? pluginName(activeId, options) : baseLabel}</span>
        </div>
      </div>
      <div className="plugin-option-grid">
        <button
          aria-pressed={activeId === null}
          className={`plugin-option ${activeId === null ? "active" : ""}`}
          onClick={() => onChange(null)}
          type="button"
        >
          <span>{baseLabel}</span>
          <em>built-in</em>
          <small>{baseHint}</small>
        </button>
        {options.map((plugin) => (
          <button
            aria-pressed={activeId === plugin.id}
            className={`plugin-option ${activeId === plugin.id ? "active" : ""}`}
            key={plugin.id}
            onClick={() => onChange(activeId === plugin.id ? null : plugin.id)}
            type="button"
          >
            <span>{plugin.name}</span>
            <em>{plugin.capability}</em>
            <small>{plugin.description}</small>
          </button>
        ))}
      </div>
    </section>
  );
}

function ReaderArticle({
  article,
  detailViewPlugin,
  readerTypography,
  onToggleRead,
  onToggleSaved,
}: {
  article: Article;
  detailViewPlugin: DetailViewPluginId | null;
  readerTypography: ReaderTypography;
  onToggleRead: (article: Article) => void;
  onToggleSaved: (article: Article) => void;
}) {
  const sanitizedHtml = useMemo(
    () => (article.contentHtml ? sanitizeArticleHtml(article.contentHtml) : ""),
    [article.contentHtml],
  );
  const readerVideos = useMemo(
    () => collectReaderVideos(article, sanitizedHtml),
    [article, sanitizedHtml],
  );

  return (
    <article
      className="reader-article"
      data-detail-view-plugin={detailViewPlugin ?? "native"}
      data-typography={readerTypography}
    >
      <div className="reader-kicker">
        <span>{article.sourceTitle}</span>
        <span>{formatDate(article.publishedAt ?? article.createdAt)}</span>
      </div>
      <h2>{article.title}</h2>
      {article.author ? <p className="byline">{article.author}</p> : null}
      <ArticleCustomFields article={article} />
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
      <ReaderVideoPlayer videos={readerVideos} />
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

function ReaderVideoPlayer({ videos }: { videos: ReaderVideo[] }) {
  if (videos.length === 0) {
    return null;
  }
  return (
    <section className="reader-video-stack" aria-label="Article video player">
      {videos.map((video) => (
        <figure className="reader-video-frame" key={`${video.kind}:${video.url}`}>
          {video.kind === "file" ? (
            <video controls preload="metadata" poster={video.poster ?? undefined}>
              <source src={video.url} type={video.mimeType} />
              <a href={video.url} rel="noreferrer" target="_blank">
                Open video
              </a>
            </video>
          ) : (
            <iframe
              allow="accelerometer; autoplay; clipboard-write; encrypted-media; fullscreen; picture-in-picture"
              allowFullScreen
              loading="lazy"
              referrerPolicy="no-referrer"
              sandbox="allow-same-origin allow-scripts allow-presentation allow-popups"
              src={video.url}
              title={video.label}
            />
          )}
          <figcaption>
            <span>{video.kind === "file" ? "Video" : "Embedded video"}</span>
            <a href={video.url} rel="noreferrer" target="_blank">
              Open
            </a>
          </figcaption>
        </figure>
      ))}
    </section>
  );
}

function ArticleCustomFields({ article }: { article: { tagsJson?: string | null } }) {
  const fields = parseArticleCustomFields(article.tagsJson);
  if (fields.length === 0) return null;
  return (
    <dl className="article-custom-fields">
      {fields.map((field) => (
        <div key={field.key}>
          <dt>{field.label}</dt>
          <dd>{field.value}</dd>
        </div>
      ))}
    </dl>
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

function themeStatusLabel(mode: ThemeMode): string {
  const resolved = themeLabel(resolveThemeMode(mode));
  return mode === "system" ? `System / ${resolved}` : resolved;
}

function viewLabel(mode: ViewMode): string {
  if (mode === "reader") {
    return "Reader";
  }
  if (mode === "sources") {
    return "Sources";
  }
  if (mode === "hub") {
    return "Hub";
  }
  return "Settings";
}

function sourceInputModeLabel(mode: SourceInputMode): string {
  if (mode === "rsshub") {
    return "RSSHub";
  }
  if (mode === "xpath") {
    return "XPath";
  }
  return "RSS/Atom";
}

function sourceInputModeKind(mode: SourceInputMode): string {
  if (mode === "rsshub") {
    return "Route";
  }
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

function isViewPluginPack(pack: { kind: string }): boolean {
  return (
    pack.kind === "app-ui-theme" ||
    pack.kind === "source-list-view" ||
    pack.kind === "detail-view"
  );
}

const sourceFamilyOrder = ["Runtime", "Forum", "Video", "Article", "Other"];
const viewSlotOrder = ["App UI", "Source List", "Detail View"];

function sourcePackFamilies(pack: PluginPack): Set<string> {
  const families = new Set<string>();
  if (isRuntimeSourcePluginPack(pack)) {
    families.add("Runtime");
    return families;
  }
  for (const candidate of pluginCandidates(pack)) {
    const pageType = candidate.pageType.toLowerCase();
    if (pageType.includes("forum")) families.add("Forum");
    else if (pageType.includes("video")) families.add("Video");
    else if (pageType.includes("article")) families.add("Article");
    else families.add("Other");
  }
  return families;
}

function viewPluginCategory(pack: { kind: string }): string | null {
  if (pack.kind === "app-ui-theme") {
    return "App UI";
  }
  if (pack.kind === "source-list-view") {
    return "Source List";
  }
  if (pack.kind === "detail-view") {
    return "Detail View";
  }
  return null;
}

function pluginPackFromXPathRulePack(pack: XPathRulePack): PluginPack {
  return {
    id: pack.id,
    name: pack.name,
    version: pack.version,
    apiVersion: pack.apiVersion,
    kind: pack.kind,
    registry: pack.registry,
    trust: pack.trust,
    description: pack.description,
    logo: pack.logo,
    capabilities: pack.capabilities,
    authors: pack.authors,
    xpath: pack,
  };
}

function pluginCandidates(pack: PluginPack): XPathRuleCandidate[] {
  return pack.xpath?.candidates ?? [];
}

function pluginParameters(pack: PluginPack | null): PluginParameters | null {
  return pack?.xpath?.parameters ?? null;
}

function pluginAuth(pack: PluginPack | null): PluginAuth | null {
  return pack?.xpath?.auth ?? null;
}

function pluginTokens(pack: PluginPack): Record<string, string> | null | undefined {
  return pack.view?.tokens;
}

function isRuntimeSourcePluginPack(pack: { kind: string }): boolean {
  return pack.kind === "runtime-source-plugin";
}

function pluginKindLabel(pack: PluginPack): string {
  if (isRuntimeSourcePluginPack(pack)) {
    return "Runtime Source";
  }
  if (isViewPluginPack(pack)) {
    return viewPluginCategory(pack) ?? "View plugin";
  }
  const count = pluginCandidates(pack).length;
  return `${count} rule${count !== 1 ? "s" : ""}`;
}

function pluginMetaLabel(pack: PluginPack): string {
  if (isRuntimeSourcePluginPack(pack)) {
    return pack.runtime
      ? `${pack.runtime.runtime.engine}${pack.runtime.runtime.package ? ` · ${pack.runtime.runtime.package}` : ""}`
      : "Runtime source";
  }
  if (isViewPluginPack(pack)) {
    return pack.capabilities.join(", ");
  }
  return pluginCandidates(pack).map((candidate) => candidate.pageType).join(", ");
}

function marketplacePackKey(pack: MarketplacePluginPack): string {
  return `${pack.sourceMarketId ?? "local"}:${pack.id}:${pack.version}`;
}

function pluginMarketLabel(pack: MarketplacePluginPack): string {
  if (!pack.sourceMarketId) {
    return "Local";
  }
  const repository = pack.sourceMarketRepository ?? "";
  const owner = repository
    .replace(/^https?:\/\/github.com\//, "")
    .replace(/\.git$/, "")
    .split("/")[0];
  const market = owner || pack.sourceMarketName || pack.sourceMarketId;
  return `${market} · v${pack.version}`;
}

function appUiPluginFromPack(pack: { id: string }): AppUiPluginId {
  if (pack.id === "official.cyberpunk-ui.view") {
    return "cyberpunk";
  }
  return pack.id;
}

function sourceListPluginFromPack(pack: { id: string }): SourceListPluginId {
  if (pack.id === "official.social-source-list.view") {
    return "social-stream";
  }
  if (pack.id === "official.dense-radar-source-list.view") {
    return "dense-radar";
  }
  return "image-board";
}

function detailViewPluginFromPack(pack: { id: string }): DetailViewPluginId {
  if (pack.id === "official.cinema-detail.view") {
    return "cinema";
  }
  if (pack.id === "official.focus-detail.view") {
    return "focus";
  }
  if (pack.id === "official.research-detail.view") {
    return "research";
  }
  return "magazine";
}

function viewPluginIdFromPack(pack: { id: string; kind: string }): string {
  if (pack.id.startsWith("view.")) {
    return pack.id.slice("view.".length);
  }
  if (pack.kind === "app-ui-theme") {
    return appUiPluginFromPack(pack);
  }
  if (pack.kind === "source-list-view") {
    return sourceListPluginFromPack(pack);
  }
  return detailViewPluginFromPack(pack);
}

function withBuiltinViewPacks(packs: MarketplacePluginPack[]): MarketplacePluginPack[] {
  return packs;
}

function viewPluginDefinitionsForKind(
  packs: MarketplacePluginPack[],
  kind: string,
): ViewPluginDefinition<string>[] {
  const byId = new Map<string, ViewPluginDefinition<string>>();
  for (const pack of packs) {
    if (pack.kind !== kind) {
      continue;
    }
    const id = viewPluginIdFromPack(pack);
    byId.set(id, {
      id,
      name: pack.name,
      description: pack.description,
      capability: pack.capabilities[0] ?? pluginKindLabel(pack),
      tokens: pluginTokens(pack),
    });
  }
  return [...byId.values()];
}

const appUiTokenCssVars: Record<string, string> = {
  colorBg: "--color-bg",
  colorBgAccent: "--color-bg-accent",
  colorPanel: "--color-panel",
  colorPanelStrong: "--color-panel-strong",
  colorPanelSoft: "--color-panel-soft",
  colorPanelMuted: "--color-panel-muted",
  colorText: "--color-text",
  colorHeading: "--color-heading",
  colorMuted: "--color-muted",
  colorFaint: "--color-faint",
  colorBorder: "--color-border",
  colorBorderStrong: "--color-border-strong",
  colorBrand: "--color-brand",
  colorBrandContrast: "--color-brand-contrast",
  colorAction: "--color-action",
  colorActionHover: "--color-action-hover",
  colorActionContrast: "--color-action-contrast",
  colorSuccess: "--color-success",
  colorWarning: "--color-warning",
  colorDanger: "--color-danger",
  colorDangerBg: "--color-danger-bg",
  colorDangerBorder: "--color-danger-border",
  colorSelectedRing: "--color-selected-ring",
  colorFill: "--color-fill",
  colorLine: "--color-line",
  colorLineSoft: "--color-line-soft",
  shadowPanel: "--shadow-panel",
  shadowSoft: "--shadow-soft",
};

function cssVariablesFromAppUiTokens(tokens?: Record<string, string> | null): CSSProperties {
  if (!tokens) {
    return {};
  }
  const style: Record<string, string> = {};
  for (const [token, cssVariable] of Object.entries(appUiTokenCssVars)) {
    const value = tokens[token];
    if (typeof value === "string" && value.trim()) {
      style[cssVariable] = value;
    }
  }
  return style as CSSProperties;
}

function pluginName<T extends string>(
  pluginId: T,
  plugins: ViewPluginDefinition<T>[],
): string {
  return plugins.find((plugin) => plugin.id === pluginId)?.name ?? pluginId;
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

function readInitialPluginId<T extends string>(
  storageKey: string,
  plugins: ViewPluginDefinition<T>[],
): T | null {
  const stored = localStorage.getItem(storageKey);
  if (plugins.some((plugin) => plugin.id === stored)) {
    return stored as T;
  }
  return null;
}

function readInitialInstalledViewPlugins(): string[] {
  const stored = localStorage.getItem(installedViewPluginsStorageKey);
  if (!stored) {
    return [];
  }
  try {
    const parsed = JSON.parse(stored);
    return Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === "string") : [];
  } catch {
    return [];
  }
}

function readInitialInstalledViewPluginVersions(): Record<string, string> {
  const stored = localStorage.getItem(installedViewPluginVersionsStorageKey);
  if (!stored) {
    return {};
  }
  try {
    const parsed = JSON.parse(stored);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }
    return Object.fromEntries(
      Object.entries(parsed as Record<string, unknown>).filter(
        (entry): entry is [string, string] =>
          typeof entry[0] === "string" && typeof entry[1] === "string",
      ),
    );
  } catch {
    return {};
  }
}

function readInitialSourceListViewBySource(): Record<string, string> {
  const stored = localStorage.getItem(sourceListViewBySourceStorageKey);
  if (!stored) {
    return {};
  }
  try {
    const parsed = JSON.parse(stored);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return Object.fromEntries(
        Object.entries(parsed as Record<string, unknown>).filter(
          ([, value]) => typeof value === "string",
        ) as [string, string][],
      );
    }
    return {};
  } catch {
    return {};
  }
}

function isSourceListPluginId(value: string): value is SourceListPluginId {
  return sourceListPlugins.some((plugin) => plugin.id === value);
}

function resolveThemeMode(mode: ThemeMode): "light" | "dark" {
  if (mode === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return mode;
}

function readInitialAppUiThemeByMode(): AppUiThemeByMode {
  const result: AppUiThemeByMode = { light: null, dark: null };
  const stored = localStorage.getItem(appUiThemeByModeStorageKey);
  if (!stored) {
    return result;
  }
  try {
    const parsed = JSON.parse(stored) as Record<string, unknown>;
    for (const mode of ["light", "dark"] as const) {
      const value = parsed?.[mode];
      if (typeof value === "string") {
        result[mode] = value;
      }
    }
  } catch {
    return result;
  }
  return result;
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

function applyAppUiPlugin(pluginId: AppUiPluginId | null): void {
  document.documentElement.dataset.appUiPlugin = pluginId ?? "native";
}

function persistNullablePlugin(storageKey: string, pluginId: string | null): void {
  if (pluginId) {
    localStorage.setItem(storageKey, pluginId);
    return;
  }
  localStorage.removeItem(storageKey);
}

function comparePluginVersions(nextVersion: string, currentVersion: string): number {
  const nextParts = versionParts(nextVersion);
  const currentParts = versionParts(currentVersion);
  const length = Math.max(nextParts.length, currentParts.length);
  for (let index = 0; index < length; index += 1) {
    const next = nextParts[index] ?? 0;
    const current = currentParts[index] ?? 0;
    if (next !== current) {
      return next > current ? 1 : -1;
    }
  }
  return nextVersion.localeCompare(currentVersion);
}

function versionParts(version: string): number[] {
  return version
    .replace(/^[vV]/, "")
    .split(/[.-]/)
    .map((part) => Number.parseInt(part, 10))
    .filter((part) => Number.isFinite(part));
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
  showTitle = true,
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
  showTitle?: boolean;
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
      {showTitle ? (
        <input
          aria-label="XPath source title"
          disabled={isBusy}
          onChange={(event) => onTitleChange(event.currentTarget.value)}
          placeholder="Source title"
          value={title}
        />
      ) : null}
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
        <label className="selector-input">
          <span>Cookie</span>
          <input
            disabled={isBusy}
            onChange={(event) =>
              onSelectorsChange({
                ...selectors,
                cookie: event.currentTarget.value,
              })
            }
            placeholder='name=value; ... or {"name":"value"} or $ENV_NAME'
            type="password"
            value={selectors.cookie ?? ""}
          />
          <small className="selector-hint">Sent as the Cookie header for list and detail pages; JSON objects are converted automatically.</small>
        </label>
        <SelectorInput
          disabled={isBusy}
          label="Content"
          name="content"
          onChange={onSelectorsChange}
          selectors={selectors}
        />
        <SelectorInput
          disabled={isBusy}
          hint="Document-level selector on each article URL, e.g. //*[@id='postmessage_123']"
          label="Detail content"
          name="detailContent"
          onChange={onSelectorsChange}
          selectors={selectors}
        />
        <label className="selector-input">
          <span>Max items per refresh</span>
          <input
            disabled={isBusy}
            min="1"
            onChange={(event) =>
              onSelectorsChange({
                ...selectors,
                maxItems: parseOptionalPositiveInt(event.currentTarget.value),
              })
            }
            placeholder="No limit"
            type="number"
            value={selectors.maxItems ?? ""}
          />
        </label>
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
        <JsonArrayInput
          disabled={isBusy}
          label="Content cleanup"
          onChange={(contentCleanup) => onSelectorsChange({ ...selectors, contentCleanup })}
          placeholder='[{"pattern":"(?is)<aside.*?</aside>","replacement":""}]'
          value={selectors.contentCleanup ?? []}
        />
        <JsonArrayInput
          disabled={isBusy}
          label="Custom fields"
          onChange={(customFields) => onSelectorsChange({ ...selectors, customFields })}
          placeholder='[{"key":"views","label":"Views","xpath":".//span[@class=\"views\"]","scope":"item"}]'
          value={selectors.customFields ?? []}
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
              <ArticleCustomFields article={article} />
              {article.summary ? <p>{article.summary}</p> : null}
              {article.author ? <em>{article.author}</em> : null}
            </article>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function JsonArrayInput<T>({
  disabled,
  label,
  onChange,
  placeholder,
  value,
}: {
  disabled: boolean;
  label: string;
  onChange: (value: T[]) => void;
  placeholder: string;
  value: T[];
}) {
  const serialized = JSON.stringify(value, null, 2);
  const [draft, setDraft] = useState(serialized);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setDraft(serialized);
    setError(null);
  }, [serialized]);

  return (
    <label className="selector-input selector-json-input">
      <span>{label}</span>
      <textarea
        disabled={disabled}
        onChange={(event) => {
          const nextDraft = event.currentTarget.value;
          setDraft(nextDraft);
          try {
            const parsed = JSON.parse(nextDraft || "[]");
            if (!Array.isArray(parsed)) {
              setError("Expected a JSON array.");
              return;
            }
            setError(null);
            onChange(parsed as T[]);
          } catch (parseError) {
            setError(parseError instanceof Error ? parseError.message : "Invalid JSON.");
          }
        }}
        placeholder={placeholder}
        rows={5}
        value={draft}
      />
      {error ? (
        <small className="selector-hint selector-error">{error}</small>
      ) : (
        <small className="selector-hint">JSON array. Regex patterns run after XPath content extraction.</small>
      )}
    </label>
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
  const rawValue = selectors[name];
  const value = typeof rawValue === "string" || typeof rawValue === "number" ? rawValue : "";
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
        value={value}
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
    cookie: emptyToUndefined(selectors.cookie),
    content: emptyToUndefined(selectors.content),
    detailContent: emptyToUndefined(selectors.detailContent),
    contentCleanup: normalizeContentCleanup(selectors.contentCleanup),
    image: emptyToUndefined(selectors.image),
    nextPage: emptyToUndefined(selectors.nextPage),
    customFields: normalizeCustomFields(selectors.customFields),
    maxItems: normalizeOptionalPositiveInt(selectors.maxItems),
    plugin: selectors.plugin,
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
    cookie: selectors.cookie?.trim() || "",
    content: selectors.content?.trim() || "",
    detailContent: selectors.detailContent?.trim() || "",
    contentCleanup: normalizeContentCleanup(selectors.contentCleanup) ?? [],
    image: selectors.image?.trim() || "",
    nextPage: selectors.nextPage?.trim() || "",
    customFields: normalizeCustomFields(selectors.customFields) ?? [],
    maxItems: normalizeOptionalPositiveInt(selectors.maxItems),
    plugin: selectors.plugin,
  };
}

function readXPathSelectorsFromSource(source: Source): XPathSelectors {
  if (!source.configJson) {
    return defaultXPathSelectors;
  }
  try {
    return normalizeXPathSelectorsForForm(JSON.parse(source.configJson) as XPathSelectors);
  } catch {
    return defaultXPathSelectors;
  }
}

function readRssHubConfigFromSource(source: Source): RssHubSourceConfig | null {
  if (!source.configJson) {
    return { route: source.url };
  }
  try {
    const config = JSON.parse(source.configJson) as RssHubSourceConfig;
    return {
      route: config.route?.trim() || source.url,
      instanceId: config.instanceId || null,
    };
  } catch {
    return { route: source.url };
  }
}

function rssHubGlobalName(settings: RssHubSettings): string {
  return (
    settings.instances.find((instance) => instance.id === settings.globalInstanceId)?.name ??
    "global"
  );
}

function emptyToUndefined(value?: string): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function cookieSummary(value?: string): string | undefined {
  const trimmed = value?.trim();
  if (!trimmed) return undefined;
  return trimmed.startsWith("$") ? trimmed : "Set";
}

function customFieldSummary(fields?: XPathCustomField[]): string | undefined {
  const normalized = normalizeCustomFields(fields);
  if (!normalized?.length) return undefined;
  return normalized.map((field) => `${field.label || field.key} (${field.scope ?? "item"})`).join(", ");
}

function normalizeContentCleanup(rules?: ContentCleanupRule[]): ContentCleanupRule[] | undefined {
  const normalized = (rules ?? [])
    .map((rule) => ({
      pattern: rule.pattern?.trim() ?? "",
      replacement: rule.replacement ?? "",
    }))
    .filter((rule) => rule.pattern);
  return normalized.length ? normalized : undefined;
}

function normalizeCustomFields(fields?: XPathCustomField[]): XPathCustomField[] | undefined {
  const normalized = (fields ?? [])
    .map((field) => ({
      key: field.key?.trim() ?? "",
      label: field.label?.trim() || undefined,
      xpath: field.xpath?.trim() ?? "",
      scope: field.scope === "detail" ? "detail" as const : "item" as const,
    }))
    .filter((field) => field.key && field.xpath);
  return normalized.length ? normalized : undefined;
}

function parseArticleCustomFields(tagsJson?: string | null): ParsedArticleCustomField[] {
  if (!tagsJson?.trim()) return [];
  try {
    const parsed = JSON.parse(tagsJson) as Record<string, unknown>;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return [];
    return Object.entries(parsed)
      .map(([key, raw]) => {
        if (typeof raw === "string") {
          return { key, label: key, value: raw };
        }
        if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
          return null;
        }
        const field = raw as Partial<ArticleCustomFieldValue>;
        const value = typeof field.value === "string" ? field.value.trim() : "";
        if (!value) return null;
        return {
          key,
          label: typeof field.label === "string" && field.label.trim() ? field.label.trim() : key,
          value,
        };
      })
      .filter((field): field is ParsedArticleCustomField => Boolean(field));
  } catch {
    return [];
  }
}

function parseOptionalPositiveInt(value: string): number | undefined {
  return normalizeOptionalPositiveInt(Number(value));
}

function normalizeOptionalPositiveInt(value?: number): number | undefined {
  if (!Number.isFinite(value)) {
    return undefined;
  }
  const normalized = Math.floor(Number(value));
  return normalized > 0 ? normalized : undefined;
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
  if (collectReaderVideos(article, article.contentHtml ?? "").length > 0) {
    return article.contentHtml || article.contentText ? "Video + body" : "Video";
  }
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

function collectReaderVideos(article: Article, html: string): ReaderVideo[] {
  const videos = new Map<string, ReaderVideo>();
  const addVideo = (candidate: ReaderVideo | null) => {
    if (candidate && !videos.has(candidate.url)) {
      videos.set(candidate.url, candidate);
    }
  };

  addVideo(videoCandidateFromUrl(article.url, "Article video", article.imageUrl));
  if (article.canonicalUrl) {
    addVideo(videoCandidateFromUrl(article.canonicalUrl, "Canonical video", article.imageUrl));
  }

  if (html && typeof DOMParser !== "undefined") {
    const document = new DOMParser().parseFromString(html, "text/html");
    document.querySelectorAll("video").forEach((video, index) => {
      const poster = cleanMediaUrl(video.getAttribute("poster")) ?? article.imageUrl;
      addVideo(videoCandidateFromUrl(video.getAttribute("src"), `Video ${index + 1}`, poster));
      video.querySelectorAll("source").forEach((source, sourceIndex) => {
        addVideo(
          videoCandidateFromUrl(
            source.getAttribute("src"),
            `Video ${index + 1}.${sourceIndex + 1}`,
            poster,
          ),
        );
      });
    });

    document.querySelectorAll("iframe").forEach((frame, index) => {
      addVideo(embedVideoCandidate(frame.getAttribute("src"), `Embedded video ${index + 1}`));
    });

    document.querySelectorAll("a[href]").forEach((anchor, index) => {
      addVideo(videoCandidateFromUrl(anchor.getAttribute("href"), `Linked video ${index + 1}`, article.imageUrl));
      addVideo(embedVideoCandidate(anchor.getAttribute("href"), `Embedded video ${index + 1}`));
    });
  }

  return [...videos.values()].slice(0, 3);
}

function videoCandidateFromUrl(
  value: string | null | undefined,
  label: string,
  poster?: string | null,
): ReaderVideo | null {
  const url = cleanMediaUrl(value);
  if (!url || !isDirectVideoUrl(url)) {
    return null;
  }
  return {
    kind: "file",
    url,
    label,
    mimeType: videoMimeType(url),
    poster: cleanMediaUrl(poster),
  };
}

function embedVideoCandidate(
  value: string | null | undefined,
  label: string,
): ReaderVideo | null {
  const url = cleanMediaUrl(value);
  const embedUrl = url ? normalizeTrustedVideoEmbedUrl(url) : null;
  return embedUrl ? { kind: "embed", url: embedUrl, label } : null;
}

function cleanMediaUrl(value: string | null | undefined): string | null {
  const trimmed = value?.trim();
  if (!trimmed) {
    return null;
  }
  try {
    const parsed = new URL(trimmed, window.location.href);
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      return null;
    }
    return parsed.toString();
  } catch {
    return null;
  }
}

function isDirectVideoUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return /\.(mp4|webm|ogv|ogg|mov|m4v|m3u8)(?:$|\?)/i.test(parsed.pathname + parsed.search);
  } catch {
    return false;
  }
}

function videoMimeType(url: string): string | undefined {
  const path = new URL(url).pathname.toLowerCase();
  if (path.endsWith(".mp4") || path.endsWith(".m4v")) return "video/mp4";
  if (path.endsWith(".webm")) return "video/webm";
  if (path.endsWith(".ogv") || path.endsWith(".ogg")) return "video/ogg";
  if (path.endsWith(".m3u8")) return "application/vnd.apple.mpegurl";
  return undefined;
}

function normalizeTrustedVideoEmbedUrl(url: string): string | null {
  try {
    const parsed = new URL(url);
    const host = parsed.hostname.replace(/^www\./, "");
    if (host === "youtube.com" || host === "youtube-nocookie.com") {
      const id = parsed.pathname.startsWith("/embed/")
        ? parsed.pathname.split("/")[2]
        : parsed.searchParams.get("v");
      return id ? `https://www.youtube-nocookie.com/embed/${encodeURIComponent(id)}` : null;
    }
    if (host === "youtu.be") {
      const id = parsed.pathname.split("/").filter(Boolean)[0];
      return id ? `https://www.youtube-nocookie.com/embed/${encodeURIComponent(id)}` : null;
    }
    if (host === "player.vimeo.com" && parsed.pathname.startsWith("/video/")) {
      return parsed.toString();
    }
    if (host === "vimeo.com") {
      const id = parsed.pathname.split("/").filter(Boolean)[0];
      return id ? `https://player.vimeo.com/video/${encodeURIComponent(id)}` : null;
    }
    if (host === "player.bilibili.com") {
      return parsed.toString();
    }
    return null;
  } catch {
    return null;
  }
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
