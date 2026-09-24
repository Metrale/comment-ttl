<!-- 2026-09-24: html comment kind -->
<script lang="ts">
  // 2026-09-24: js line comment kind
  /* js block comment kind */
  const s = "<!-- DECOY -->"; // trailing
  let tpl = `${ x } <!-- DECOY -->`;
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div on:click={handle}>// DECOY in markup</div>

<style>
  /* 2026-09-24: css comment kind */
  .a { content: "/* DECOY */"; }
</style>
