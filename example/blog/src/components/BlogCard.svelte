<script>
  let { title, excerpt, date, slug, tags = [], readTime = '5 min read' } = $props();
</script>

<article class="blog-card">
  <div class="meta">
    <time>{date}</time>
    <span class="read-time">{readTime}</span>
  </div>

  <h3><a href={`/posts/${slug}`}>{title}</a></h3>
  <p>{excerpt}</p>

  {#if tags.length > 0}
    <div class="tags">
      {#each tags as tag}
        <span class="tag">{tag}</span>
      {/each}
    </div>
  {/if}
</article>

<style>
  .blog-card {
    border: 1px solid #e9ecef;
    border-radius: 8px;
    padding: 1.5rem;
    margin-bottom: 1rem;
    background: white;
    box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
    transition: box-shadow 0.2s ease;
  }

  .blog-card:hover {
    box-shadow: 0 4px 10px rgba(0, 0, 0, 0.15);
  }

  .meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.8rem;
    opacity: 0.7;
    margin-bottom: 1rem;
    color: #6c757d;
  }

  .read-time {
    background: #f8f9fa;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    font-size: 0.7rem;
    color: #495057;
  }

  h3 {
    margin: 0 0 0.5rem 0;
    font-size: 1.2rem;
  }

  h3 a {
    color: #2c3e50;
    text-decoration: none;
  }

  h3 a:hover {
    color: #3498db;
  }

  p {
    line-height: 1.5;
    margin-bottom: 1rem;
    opacity: 0.8;
    color: #495057;
  }

  .tags {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .tag {
    background: #e9ecef;
    color: #495057;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    font-size: 0.7rem;
  }
</style>
