import { useEffect, useMemo, useState } from "react";
import type { CSSProperties, FormEvent, KeyboardEvent, PointerEvent } from "react";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import "./App.css";

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
type ArticleDensity = "comfortable" | "compact";
type ReaderTypography = "system" | "serif" | "large";
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

type ParsedArticle = {
  title: string;
  url: string;
  summary?: string | null;
  publishedAt?: string | null;
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

const themeStorageKey = "feader.theme";
const densityStorageKey = "feader.articleDensity";
const paneStorageKey = "feader.paneWidths";
const readerTypographyStorageKey = "feader.readerTypography";
const feedGroupStorageKey = "feader.feedGroups";
const builtInTestFeedUrl = "https://www.appinn.com/feed/";
const defaultPaneWidths: PaneWidths = {
  sidebar: 240,
  timeline: 520,
};
const paneBounds: Record<PaneKey, { min: number; max: number }> = {
  sidebar: { min: 220, max: 300 },
  timeline: { min: 360, max: 620 },
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
      return testModeArticleState.slice(0, 3).map(({ title, url, summary, publishedAt }) => ({
        title,
        url,
        summary,
        publishedAt,
      })) as T;
    case "add_xpath_source":
      throw new Error("XPath test mode is read-only. Use the Tauri app to validate XPath sources.");
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
  const [sources, setSources] = useState<Source[]>([]);
  const [articles, setArticles] = useState<Article[]>([]);
  const [selectedSourceId, setSelectedSourceId] = useState<number | undefined>();
  const [selectedArticleId, setSelectedArticleId] = useState<number | undefined>();
  const [filterMode, setFilterMode] = useState<FilterMode>("all");
  const [sourceInputMode, setSourceInputMode] = useState<SourceInputMode>("rss");
  const [activeView, setActiveView] = useState<ViewMode>("reader");
  const [showSourceComposer, setShowSourceComposer] = useState(false);
  const [themeMode, setThemeMode] = useState<ThemeMode>(() => readInitialThemeMode());
  const [articleDensity, setArticleDensity] = useState<ArticleDensity>(() =>
    readInitialArticleDensity(),
  );
  const [readerTypography, setReaderTypography] = useState<ReaderTypography>(() =>
    readInitialReaderTypography(),
  );
  const [paneWidths, setPaneWidths] = useState<PaneWidths>(() => readInitialPaneWidths());
  const [feedUrl, setFeedUrl] = useState("");
  const [xpathTitle, setXPathTitle] = useState("");
  const [xpathSelectors, setXPathSelectors] = useState<XPathSelectors>(defaultXPathSelectors);
  const [xpathPreview, setXPathPreview] = useState<ParsedArticle[]>([]);
  const [editingTitle, setEditingTitle] = useState("");
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

  useEffect(() => {
    void loadData();
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
    localStorage.setItem(densityStorageKey, articleDensity);
  }, [articleDensity]);

  useEffect(() => {
    localStorage.setItem(readerTypographyStorageKey, readerTypography);
  }, [readerTypography]);

  useEffect(() => {
    localStorage.setItem(paneStorageKey, JSON.stringify(paneWidths));
  }, [paneWidths]);

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

  async function handleAddFeed(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const url = feedUrl.trim();
    if (!url) {
      setStatus("Enter a feed URL first.");
      return;
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
      setXPathPreview([]);
      setShowSourceComposer(false);
      setSelectedSourceId(source.id);
      setFilterMode("all");
      await loadData(source.id, "all", undefined);
      setStatus(`Added ${source.title}`);
    });
  }

  async function handlePreviewXPath(): Promise<void> {
    const url = feedUrl.trim();
    if (!url) {
      setStatus("Enter a page URL first.");
      return;
    }

    await runTask("Previewing XPath", async () => {
      const preview = await invoke<ParsedArticle[]>("preview_xpath_source", {
        request: {
          url,
          selectors: compactXPathSelectors(xpathSelectors),
        },
      });
      setXPathPreview(preview);
      setStatus(`Preview extracted ${preview.length} articles`);
    });
  }

  async function handleSelectSource(sourceId?: number): Promise<void> {
    setSelectedSourceId(sourceId);
    setEditingTitle("");
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

  async function handleRenameSource(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (!selectedSource) {
      return;
    }

    await runTask("Renaming feed", async () => {
      await invoke<Source>("update_source_title", {
        request: {
          sourceId: selectedSource.id,
          title: editingTitle || selectedSource.title,
        },
      });
      setEditingTitle("");
      await loadData(selectedSourceId, filterMode, selectedArticleId);
      setStatus("Feed renamed");
    });
  }

  async function handleDeleteSource(): Promise<void> {
    if (!selectedSource) {
      return;
    }
    const confirmed = window.confirm(`Delete "${selectedSource.title}" and its articles?`);
    if (!confirmed) {
      return;
    }

    await runTask("Deleting feed", async () => {
      await invoke("delete_source", { sourceId: selectedSource.id });
      setSelectedSourceId(undefined);
      setSelectedArticleId(undefined);
      await loadData(undefined, filterMode, undefined);
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
    if (event.key !== "Enter" && event.key !== " ") {
      return;
    }

    event.preventDefault();
    setSelectedArticleId(articleId);
  }

  function handleAppKeyDown(event: KeyboardEvent<HTMLElement>): void {
    if (activeView !== "reader" || isTextInputTarget(event.target)) {
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

    if (event.key.toLowerCase() === "r") {
      event.preventDefault();
      void handleToggleRead(selectedArticle);
    }

    if (event.key.toLowerCase() === "s") {
      event.preventDefault();
      void handleToggleSaved(selectedArticle);
    }
  }

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
    setArticleDensity("comfortable");
    localStorage.removeItem(paneStorageKey);
    localStorage.removeItem(densityStorageKey);
  }

  async function runTask(label: string, task: () => Promise<void>): Promise<void> {
    setIsBusy(true);
    setStatus(label);
    try {
      await task();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
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
      onKeyDown={handleAppKeyDown}
      style={shellStyle}
    >
      <IconRail
        activeView={activeView}
        onSelectView={setActiveView}
        themeMode={themeMode}
        onCycleTheme={() => setThemeMode((mode) => nextThemeMode(mode))}
      />
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
          <DensityControl density={articleDensity} onChange={setArticleDensity} />
          <div className="status-line">{status}</div>
        </div>

        <div className={`story-list ${articleDensity}`}>
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
                role="button"
                tabIndex={0}
              >
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

      <PaneResizer
        label="Resize reader panel"
        onPointerDown={(event) => handlePaneResizeStart("timeline", event)}
      />
      <aside className="reader-panel" aria-label="Reader panel">
        {selectedArticle ? (
          <article className="reader-article" data-typography={readerTypography}>
            <div className="reader-kicker">
              <span>{selectedArticle.sourceTitle}</span>
              <span>{formatDate(selectedArticle.publishedAt ?? selectedArticle.createdAt)}</span>
            </div>
            <h2>{selectedArticle.title}</h2>
            {selectedArticle.author ? <p className="byline">{selectedArticle.author}</p> : null}
            <div className="reader-actions">
              <button onClick={() => void handleToggleRead(selectedArticle)} type="button">
                {selectedArticle.read ? "Mark unread" : "Mark read"}
              </button>
              <button onClick={() => void handleToggleSaved(selectedArticle)} type="button">
                {selectedArticle.saved ? "Unsave" : "Save"}
              </button>
              <a href={selectedArticle.url} rel="noreferrer" target="_blank">
                Original
              </a>
            </div>
            <dl className="reader-meta">
              <dt>Source</dt>
              <dd>{selectedArticle.sourceTitle}</dd>
              <dt>Published</dt>
              <dd>{formatDate(selectedArticle.publishedAt ?? selectedArticle.createdAt)}</dd>
              <dt>Body</dt>
              <dd>{articleBodyState(selectedArticle)}</dd>
              {selectedArticle.canonicalUrl ? (
                <>
                  <dt>Canonical</dt>
                  <dd>{selectedArticle.canonicalUrl}</dd>
                </>
              ) : null}
            </dl>
            {selectedArticle.imageUrl ? (
              <img alt="" className="reader-image" src={selectedArticle.imageUrl} />
            ) : null}
            <div className="reader-body">
              {selectedArticle.contentText ? (
                <p>{selectedArticle.contentText}</p>
              ) : selectedArticle.contentHtml ? (
                <p>{stripHtml(selectedArticle.contentHtml)}</p>
              ) : selectedArticle.summary ? (
                <p>{stripHtml(selectedArticle.summary)}</p>
              ) : (
                <p>{articleBodyFallback(selectedArticle)}</p>
              )}
            </div>
          </article>
        ) : (
          <section className="empty-state">
            <h2>No article selected</h2>
            <p>Select an article from the queue.</p>
          </section>
        )}

        <section className="source-panel">
          <div className="panel-heading">
            <span>Source</span>
            <span>{selectedSource ? sourceHealth(selectedSource) : "All feeds"}</span>
          </div>
          {selectedSource ? (
            <>
              <SourceHealthStrip source={selectedSource} />
              <form className="rename-form" onSubmit={handleRenameSource}>
                <input
                  aria-label="Source title"
                  disabled={isBusy}
                  onChange={(event) => setEditingTitle(event.currentTarget.value)}
                  placeholder={selectedSource.title}
                  value={editingTitle}
                />
                <button disabled={isBusy} type="submit">
                  Rename
                </button>
              </form>
              <dl>
                <dt>URL</dt>
                <dd>{selectedSource.url}</dd>
                <dt>Kind</dt>
                <dd>{selectedSource.kind}</dd>
                <dt>Articles</dt>
                <dd>{selectedSource.articleCount}</dd>
                <dt>Unread</dt>
                <dd>{selectedSource.unreadCount}</dd>
                <dt>Last refresh</dt>
                <dd>{formatDate(selectedSource.lastFetchedAt)}</dd>
                <dt>Status</dt>
                <dd>{sourceDiagnostic(selectedSource)}</dd>
              </dl>
              {selectedSource.lastError ? (
                <p className="error-text">{selectedSource.lastError}</p>
              ) : null}
              <button
                className="danger-action"
                disabled={isBusy}
                onClick={handleDeleteSource}
                type="button"
              >
                Delete feed
              </button>
            </>
          ) : (
            <p>All feeds selected.</p>
          )}
        </section>
      </aside>
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
                        isBusy={isBusy}
                        onPreview={() => void handlePreviewXPath()}
                        onSelectorsChange={setXPathSelectors}
                        onTitleChange={setXPathTitle}
                        preview={xpathPreview}
                        selectors={xpathSelectors}
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

            <article className="settings-card">
              <div className="panel-heading">
                <span>Workspace</span>
                <span>{articleDensityLabel(articleDensity)}</span>
              </div>
              <DensityControl density={articleDensity} onChange={setArticleDensity} />
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

function DensityControl({
  density,
  onChange,
}: {
  density: ArticleDensity;
  onChange: (density: ArticleDensity) => void;
}) {
  return (
    <div className="density-control" role="group" aria-label="Article density">
      {(["comfortable", "compact"] as const).map((nextDensity) => (
        <button
          className={density === nextDensity ? "active" : ""}
          key={nextDensity}
          onClick={() => onChange(nextDensity)}
          type="button"
        >
          {articleDensityLabel(nextDensity)}
        </button>
      ))}
    </div>
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

function articleDensityLabel(density: ArticleDensity): string {
  if (density === "compact") {
    return "Compact";
  }
  return "Comfortable";
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

function readInitialArticleDensity(): ArticleDensity {
  const stored = localStorage.getItem(densityStorageKey);
  if (stored === "compact" || stored === "comfortable") {
    return stored;
  }
  return "comfortable";
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
  isBusy,
  onPreview,
  onSelectorsChange,
  onTitleChange,
  preview,
  selectors,
  title,
}: {
  isBusy: boolean;
  onPreview: () => void;
  onSelectorsChange: (selectors: XPathSelectors) => void;
  onTitleChange: (title: string) => void;
  preview: ParsedArticle[];
  selectors: XPathSelectors;
  title: string;
}) {
  return (
    <section className="xpath-form">
      <input
        aria-label="XPath source title"
        disabled={isBusy}
        onChange={(event) => onTitleChange(event.currentTarget.value)}
        placeholder="Source title"
        value={title}
      />
      <SelectorInput
        disabled={isBusy}
        label="Items"
        name="items"
        onChange={onSelectorsChange}
        selectors={selectors}
      />
      <SelectorInput
        disabled={isBusy}
        label="Title"
        name="title"
        onChange={onSelectorsChange}
        selectors={selectors}
      />
      <SelectorInput
        disabled={isBusy}
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
      <button disabled={isBusy} onClick={onPreview} type="button">
        Preview
      </button>
      {preview.length > 0 ? (
        <div className="xpath-preview">
          {preview.map((article) => (
            <article key={article.url}>
              <strong>{article.title}</strong>
              <span>{article.url}</span>
            </article>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function SelectorInput({
  disabled,
  label,
  name,
  onChange,
  selectors,
}: {
  disabled: boolean;
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

export default App;
