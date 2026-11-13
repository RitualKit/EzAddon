<script lang="ts">
  import Settings from "./Settings.svelte";
</script>

<div class="setup scroll">
  <div class="intro">
    <h2>Welcome to EzAddon</h2>
    <p class="muted">Two quick steps and you can install WoW Forever addons with one click.</p>
  </div>
  <Settings setup />
</div>

<style>
  .setup { height: 100%; padding: 32px 16px; }
  .intro { max-width: 720px; margin: 0 auto 16px; }
  .intro h2 { font-size: 22px; }
  .intro p { margin: 6px 0 0; }
</style>
