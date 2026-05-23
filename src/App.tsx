import { FormEvent, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Source = {
  id: number;
  kind: string;
  title: string;
  url: string;
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

function App() {
  const [sources, setSources] = useState<Source[]>([]);
  const [articles, setArticles] = useState<Article[]>([]);
  const [selectedSourceId, setSelectedSourceId] = useState<number | undefined>();
  const [selectedArticleId, setSelectedArticleId] = useState<number | undefined>();
  const [filterMode, setFilterMode] = useState<FilterMode>("all");
  const [feedUrl, setFeedUrl] = useState("");
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
  const articleCount = sources.reduce((total, source) => total + source.articleCount, 0);

  useEffect(() => {
    void loadData();
  }, []);

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
      const source = await invoke<Source>("add_source", {
        request: { url },
      });
      setFeedUrl("");
      setSelectedSourceId(source.id);
      setFilterMode("all");
      await loadData(source.id, "all", undefined);
      setStatus(`Added ${source.title}`);
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

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">F</span>
          <div>
            <strong>Feader</strong>
            <span>{unreadCount} unread</span>
          </div>
        </div>

        <form className="feed-form" onSubmit={handleAddFeed}>
          <input
            aria-label="Feed URL"
            disabled={isBusy}
            onChange={(event) => setFeedUrl(event.currentTarget.value)}
            placeholder="https://example.com/feed.xml"
            value={feedUrl}
          />
          <button disabled={isBusy} type="submit">
            Add
          </button>
        </form>

        <button className="secondary-action" disabled={isBusy} onClick={handleRefreshAll} type="button">
          Refresh all
        </button>

        <nav className="feed-list" aria-label="Feeds">
          <button
            className={`feed-item ${selectedSourceId === undefined ? "active" : ""}`}
            onClick={() => void handleSelectSource(undefined)}
            type="button"
          >
            <span>All feeds</span>
            <small>{articleCount}</small>
          </button>
          {sources.map((source) => (
            <button
              className={`feed-item ${selectedSourceId === source.id ? "active" : ""}`}
              key={source.id}
              onClick={() => void handleSelectSource(source.id)}
              type="button"
            >
              <span>{source.title}</span>
              <small>{source.unreadCount}</small>
            </button>
          ))}
        </nav>
      </aside>

      <section className="timeline" aria-label="Reading queue">
        <header className="topbar">
          <div>
            <p className="eyebrow">{selectedSource?.kind ?? "RSS"}</p>
            <h1>{selectedSource?.title ?? "Reading queue"}</h1>
          </div>
          <div className="topbar-actions">
            <button
              className="secondary-action"
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

        <div className="filter-tabs" role="tablist" aria-label="Article filters">
          {(["all", "unread", "saved"] as const).map((mode) => (
            <button
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

        <div className="status-line">{status}</div>

        <div className="story-list">
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
                onClick={() => setSelectedArticleId(article.id)}
              >
                <div className="story-meta">
                  <span>{article.sourceTitle}</span>
                  <span>{formatDate(article.publishedAt ?? article.createdAt)}</span>
                </div>
                <h2>{article.title}</h2>
                {article.summary ? <p>{stripHtml(article.summary)}</p> : null}
                <div className="story-actions">
                  <button onClick={() => void handleToggleRead(article)} type="button">
                    {article.read ? "Unread" : "Read"}
                  </button>
                  <button onClick={() => void handleToggleSaved(article)} type="button">
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

      <aside className="reader-panel" aria-label="Reader panel">
        {selectedArticle ? (
          <article className="reader-article">
            <div className="story-meta">
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
            <div className="reader-body">
              {selectedArticle.contentText ? (
                <p>{selectedArticle.contentText}</p>
              ) : selectedArticle.contentHtml ? (
                <p>{stripHtml(selectedArticle.contentHtml)}</p>
              ) : selectedArticle.summary ? (
                <p>{stripHtml(selectedArticle.summary)}</p>
              ) : (
                <p>No local article body was provided by this feed.</p>
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
          <p className="eyebrow">Source</p>
          {selectedSource ? (
            <>
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
                <dt>Articles</dt>
                <dd>{selectedSource.articleCount}</dd>
                <dt>Unread</dt>
                <dd>{selectedSource.unreadCount}</dd>
                <dt>Last refresh</dt>
                <dd>{formatDate(selectedSource.lastFetchedAt)}</dd>
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
    </main>
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
