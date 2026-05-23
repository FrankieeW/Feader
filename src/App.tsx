import "./App.css";

const feeds = [
  { name: "Open Web", count: 18 },
  { name: "AI Research", count: 9 },
  { name: "Ethereum", count: 14 },
  { name: "DAO Governance", count: 7 },
];

const stories = [
  {
    source: "Protocol Watch",
    title: "Governance digest: validator economics and client diversity",
    summary:
      "AI summary identifies three proposals worth tracking, with one likely to affect staking operators this week.",
    tag: "Web3",
    time: "12 min ago",
  },
  {
    source: "Frontier Models",
    title: "New agent workflows for long-running research tasks",
    summary:
      "Clustered with four related posts. Feader suggests reading the benchmark notes before the opinion pieces.",
    tag: "AI",
    time: "34 min ago",
  },
  {
    source: "Open RSS",
    title: "Why portable subscriptions still matter",
    summary:
      "A concise argument for user-owned reading graphs, OPML exports, and local-first feed history.",
    tag: "RSS",
    time: "1 hr ago",
  },
];

function App() {
  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">F</span>
          <div>
            <strong>Feader</strong>
            <span>AI-native RSS</span>
          </div>
        </div>

        <nav className="feed-list" aria-label="Feeds">
          {feeds.map((feed) => (
            <button className="feed-item" key={feed.name}>
              <span>{feed.name}</span>
              <small>{feed.count}</small>
            </button>
          ))}
        </nav>
      </aside>

      <section className="timeline" aria-label="Reading queue">
        <header className="topbar">
          <div>
            <p className="eyebrow">Today</p>
            <h1>Signals worth reading</h1>
          </div>
          <button className="primary-action" type="button">
            Add feed
          </button>
        </header>

        <div className="story-list">
          {stories.map((story) => (
            <article className="story-card" key={story.title}>
              <div className="story-meta">
                <span>{story.source}</span>
                <span>{story.time}</span>
              </div>
              <h2>{story.title}</h2>
              <p>{story.summary}</p>
              <span className="story-tag">{story.tag}</span>
            </article>
          ))}
        </div>
      </section>

      <aside className="insight-panel" aria-label="AI and Web3 context">
        <section>
          <p className="eyebrow">AI Brief</p>
          <h2>3 themes rising</h2>
          <p>
            Feader grouped 41 unread items into governance, agent tooling, and
            portable identity. Start with the items that change decisions.
          </p>
        </section>

        <section>
          <p className="eyebrow">Web3 Context</p>
          <h2>Wallet optional</h2>
          <p>
            Future identity support can connect ENS, Farcaster, Lens, Mirror,
            Paragraph, and DAO forums without making crypto required.
          </p>
        </section>
      </aside>
    </main>
  );
}

export default App;
