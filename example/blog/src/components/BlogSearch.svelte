<script>
  import BlogCard from './BlogCard.svelte';

  let { posts = [] } = $props();

  let searchTerm = $state('');

  let filteredPosts = $derived(posts.filter(post => {
    return post.title.toLowerCase().includes(searchTerm.toLowerCase()) ||
           post.excerpt.toLowerCase().includes(searchTerm.toLowerCase());
  }));
</script>

<div class="search-container">
  <h2>Search Posts</h2>
  <input
    type="text"
    bind:value={searchTerm}
    placeholder="Search posts..."
    class="search-input"
  />
  <p class="results-count">
    {filteredPosts.length} of {posts.length} posts
  </p>
</div>

<section class="posts-section">
  <h2>Latest Posts</h2>
  <div class="posts">
    {#each filteredPosts as post}
      <BlogCard
        title={post.title}
        excerpt={post.excerpt}
        date={post.date}
        slug={post.slug}
        tags={post.tags}
        readTime={post.readTime}
      />
    {/each}
  </div>
</section>

<style>
  .search-container {
    background: #f8f9fa;
    padding: 1.5rem;
    border-radius: 8px;
    margin-bottom: 2rem;
    border: 1px solid #e9ecef;
  }

  h2 {
    margin: 0 0 1rem 0;
    font-size: 1.2rem;
    color: #3498db;
  }

  .search-input {
    width: 100%;
    padding: 0.75rem;
    border: 1px solid #dee2e6;
    border-radius: 4px;
    background: white;
    color: #2c3e50;
    font-size: 1rem;
    margin-bottom: 1rem;
  }

  .search-input:focus {
    outline: none;
    border-color: #3498db;
    box-shadow: 0 0 0 2px rgba(52, 152, 219, 0.2);
  }

  .search-input::placeholder {
    color: #6c757d;
  }

  .results-count {
    margin: 0;
    font-size: 0.8rem;
    opacity: 0.7;
  }

  .posts-section {
    margin-bottom: 2rem;
  }

  .posts-section h2 {
    color: #3498db;
    font-size: 1.3rem;
    margin: 2rem 0 1rem 0;
  }

  .posts {
    display: grid;
    gap: 1rem;
  }
</style>
