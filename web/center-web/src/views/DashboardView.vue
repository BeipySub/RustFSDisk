<script setup lang="ts">
const disks = [
  {
    disk_id: "fixture-disk",
    runtime_status: "READY",
    import_job_status: "PENDING",
    message: "等待封盘运输盘",
  },
];
</script>

<template>
  <main class="dashboard">
    <section class="toolbar">
      <div>
        <p class="eyebrow">Center</p>
        <h1>中控端 Dashboard</h1>
      </div>
      <span class="badge">HTTP + WebSocket</span>
    </section>

    <section class="grid">
      <article v-for="disk in disks" :key="disk.disk_id" class="card">
        <h2>{{ disk.disk_id }}</h2>
        <dl>
          <div>
            <dt>运行态</dt>
            <dd>{{ disk.runtime_status }}</dd>
          </div>
          <div>
            <dt>导入任务</dt>
            <dd>{{ disk.import_job_status }}</dd>
          </div>
        </dl>
        <p>{{ disk.message }}</p>
      </article>
    </section>
  </main>
</template>
