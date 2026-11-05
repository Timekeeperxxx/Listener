import { defineStore } from 'pinia'
import { ref, shallowRef } from 'vue'
import { api, errorMessage } from '../lib/api'
import type { Download, Track } from '../types'
export interface DownloadJob { track: Track; state: 'queued' | 'downloading' | 'error'; received: number; total: number | null; error: string }
export const useDownloadsStore = defineStore('downloads', () => {
  const records = shallowRef<Download[]>([])
  const jobs = ref<DownloadJob[]>([])
  const error = ref('')
  const loading = ref(false)
  let generation = 0
  function clear() { generation++; records.value = []; jobs.value = []; error.value = ''; loading.value = false }
  async function load() {
    const request = generation
    loading.value = true; error.value = ''
    try { const result = await api.downloads(); if (request === generation) records.value = result }
    catch (cause) { if (request === generation) error.value = errorMessage(cause) }
    finally { if (request === generation) loading.value = false }
  }
  async function start(track: Track) {
    if (jobs.value.some(job => job.track.mid === track.mid && job.state !== 'error')) return
    const request = generation
    jobs.value = [...jobs.value.filter(job => job.track.mid !== track.mid), { track, state: 'queued', received: 0, total: null, error: '' }]
    try {
      const result = await api.download(track)
      if (request !== generation) return
      records.value = [result, ...records.value.filter(record => record.track.mid !== track.mid)]
      jobs.value = jobs.value.filter(job => job.track.mid !== track.mid)
    } catch (cause) {
      if (request !== generation) return
      if (typeof cause === 'object' && cause !== null && 'code' in cause && cause.code === 'DOWNLOAD_CANCELLED') {
        jobs.value = jobs.value.filter(job => job.track.mid !== track.mid)
      } else {
        const job = jobs.value.find(job => job.track.mid === track.mid)!
        job.state = 'error'; job.error = errorMessage(cause)
      }
    }
  }
  function progress(event: { mid: string; received: number; total: number | null }) {
    const job = jobs.value.find(job => job.track.mid === event.mid)
    if (job && job.state !== 'error') { job.state = 'downloading'; job.received = event.received; job.total = event.total }
  }
  async function cancel(mid: string) { try { await api.cancelDownload(mid) } catch (cause) { error.value = errorMessage(cause) } }
  async function remove(mid: string) {
    const request = generation
    error.value = ''
    try { await api.removeDownload(mid); if (request === generation) records.value = records.value.filter(record => record.track.mid !== mid) }
    catch (cause) { if (request === generation) error.value = errorMessage(cause) }
  }
  return { records, jobs, error, loading, clear, load, start, progress, cancel, remove }
})
