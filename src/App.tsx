import { FormEvent, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Source = {
  id: number;
  kind: string;
  title: string;
  url: string;
  configJson?: string | null;
  createdAt: string;
  lastFetchedAt?: string | null;
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

function App() {
  const [sources, setSources] = useState<Source[]>([]);
  const [articles, setArticles] = useState<Article[]>([]);
  const [selectedSourceId, setSelectedSourceId] = useState<number | undefined>();
  const [feedUrl, setFeedUrl] = useState("");
  const [status, setStatus] = useState("Ready");
  const [isBusy, setIsBusy] = useState(false);

  const selectedSource = useMemo(
    () => sources.find((source) => source.id === selectedSourceId),
    [selectedSourceId, sources],
  );
  const unreadCount = sources.reduce((total, source) => total + source.unreadCount, 0);
  const articleCount = sources.reduce((total, source) => total + source.articleCount, 0);

  useEffect(() => {
    void loadData();
  }, []);

  async function loadData(sourceId = selectedSourceId): Promise<void> {
    const [nextSources, nextArticles] = await Promise.all([
      invoke<Source[]>("list_sources"),
      invoke<Article[]>("list_articles", {
        filter: sourceId ? { sourceId } satisfies ArticleFilter : null,
      }),
    ]);
    setSources(nextSources);
    setArticles(nextArticles);
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
      await invoke<Article[]>("refresh_source", { sourceId: source.id });
      await loadData(source.id);
      setStatus(`Added ${source.title}`);
    });
  }

  async function handleRefreshSource(sourceId: number): Promise<void> {
    await runTask("Refreshing feed", async () => {
      await invoke<Article[]>("refresh_source", { sourceId });
      await loadData(selectedSourceId);
      setStatus("Feed refreshed");
    });
  }

  async function handleSelectSource(sourceId?: number): Promise<void> {
    setSelectedSourceId(sourceId);
    const nextArticles = await invoke<Article[]>("list_articles", {
      filter: sourceId ? { sourceId } satisfies ArticleFilter : null,
    });
    setArticles(nextArticles);
  }

  async function handleToggleRead(article: Article): Promise<void> {
    await invoke("mark_article_read", {
      articleId: article.id,
      read: !article.read,
    });
    await loadData(selectedSourceId);
  }

  async function handleToggleSaved(article: Article): Promise<void> {
    await invoke("save_article", {
      articleId: article.id,
      saved: !article.saved,
    });
    await loadData(selectedSourceId);
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
          <button
            className="primary-action"
            disabled={isBusy || !selectedSourceId}
            onClick={() => selectedSourceId && void handleRefreshSource(selectedSourceId)}
            type="button"
          >
            Refresh
          </button>
        </header>

        <div className="status-line">{status}</div>

        <div className="story-list">
          {articles.length === 0 ? (
            <section className="empty-state">
              <h2>No articles yet</h2>
              <p>Add an RSS or Atom feed, then refresh it to populate the local reader.</p>
            </section>
          ) : (
            articles.map((article) => (
              <article className={`story-card ${article.read ? "read" : ""}`} key={article.id}>
                <div className="story-meta">
                  <span>{article.sourceTitle}</span>
                  <span>{formatDate(article.publishedAt ?? article.createdAt)}</span>
                </div>
                <h2>
                  <a href={article.url} rel="noreferrer" target="_blank">
                    {article.title}
                  </a>
                </h2>
                {article.summary ? <p>{stripHtml(article.summary)}</p> : null}
                <div className="story-actions">
                  <button onClick={() => void handleToggleRead(article)} type="button">
                    {article.read ? "Unread" : "Read"}
                  </button>
                  <button onClick={() => void handleToggleSaved(article)} type="button">
                    {article.saved ? "Unsave" : "Save"}
                  </button>
                  <span className="story-tag">RSS</span>
                </div>
              </article>
            ))
          )}
        </div>
      </section>

      <aside className="insight-panel" aria-label="AI and Web3 context">
        <section>
          <p className="eyebrow">Core Flow</p>
          <h2>Local-first reader</h2>
          <p>
            Sources, articles, and read state now flow through Rust commands into a
            local SQLite database.
          </p>
        </section>

        <section>
          <p className="eyebrow">Adapter Path</p>
          <h2>RSS first</h2>
          <p>
            RSS and Atom are live first. XPath and script plugins can reuse the same
            normalized article contract.
          </p>
        </section>
      </aside>
    </main>
  );
}

function formatDate(value?: string | null): string {
  if (!value) {
    return "Unknown";
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
