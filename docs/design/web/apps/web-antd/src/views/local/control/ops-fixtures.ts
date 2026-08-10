export const controlAssets = {
  environment: '/assets/fustfs-baseline/factory-environment-v4.webp',
  rack: '/assets/fustfs-baseline/source-rack-cutout-v3.webp',
  transportNas: '/assets/fustfs-baseline/transport-nas-cutout-v3.webp',
} as const;

export const mediaDetailFixture = {
  archiveTarget: 'fustfs-archive / A-001 / 2026-07-21 / …',
  batchId: 'A-20260721-009',
  bytesIngested: '8.2 TB',
  completedPercent: 68,
  destination: '中心 B',
  eta: '00:36:18',
  mediaSerial: '…7F22',
  slot: '03',
  sourceSite: 'A-001',
  speed: '620 MB/s',
  state: '解密入库中',
  totalBytes: '12.0 TB',
  stages: [
    { label: '识别完成', state: 'finish' },
    { label: '验签完成', state: 'finish' },
    { label: '内存解密', state: 'process' },
    { label: '写入中心', state: 'process' },
    { label: '目标校验', state: 'wait' },
  ],
} as const;

export type ConflictQueueState = 'CONFLICT' | 'WAITING_KEY';

export interface ConflictQueueItem {
  batchId: string;
  label: string;
  reason: string;
  state: ConflictQueueState;
}

export const conflictSummary = {
  conflictCount: 1,
  continuingObjects: 126,
  waitingKeys: 1,
} as const;

export const conflictQueue: ConflictQueueItem[] = [
  {
    batchId: 'batch A-20260720-021',
    label: 'A-006 · 盘位 11',
    reason: '同一归档地址 · 内容不同',
    state: 'CONFLICT',
  },
  {
    batchId: '等待受控提供解密密钥',
    label: '等待密钥 · A-002 · 盘位 07',
    reason: '密钥就绪前不会进入目标写入',
    state: 'WAITING_KEY',
  },
];

export const conflictTimeline = [
  { at: '2026-07-20 10:21:13', label: '验签通过', status: 'finish' },
  { at: '2026-07-20 10:21:18', label: '解密完成', status: 'finish' },
  { at: '2026-07-20 10:21:23', label: '目标检查', status: 'finish' },
  { at: '2026-07-20 10:21:28', label: '发现冲突', status: 'error' },
  { at: '2026-07-20 10:21:31', label: '锁定完成', status: 'error' },
] as const;

export type HistoryState =
  | 'CONFLICT'
  | 'FAILED'
  | 'INGESTING'
  | 'SIGNED'
  | 'VERIFYING';

export interface ControlHistoryRow {
  batchId: string;
  bytes: string;
  key: string;
  media: string;
  result: string;
  site: string;
  state: HistoryState;
  stateLabel: string;
  time: string;
}

export const historySummary = {
  conflict: 2,
  failed: 8,
  ingesting: 3,
  signed: 231,
  total: 246,
  verifying: 2,
} as const;

export const historyRows: ControlHistoryRow[] = [
  {
    batchId: 'A-20260721-009',
    bytes: '12.0 TB',
    key: 'A-20260721-009',
    media: 'SN …7F22',
    result: '解密并写入中心',
    site: 'A-001',
    state: 'INGESTING',
    stateLabel: '入库中',
    time: '07-21 16:52',
  },
  {
    batchId: 'A-20260721-008',
    bytes: '11.8 TB',
    key: 'A-20260721-008',
    media: 'SN …51C8',
    result: '目标校验通过 · receipt 已签发',
    site: 'A-003',
    state: 'SIGNED',
    stateLabel: '完成并签发',
    time: '07-21 15:08',
  },
  {
    batchId: 'A-20260720-021',
    bytes: '4.3 TB',
    key: 'A-20260720-021',
    media: 'SN …8F2A',
    result: '禁止覆盖 · receipt 暂不签发',
    site: 'A-006',
    state: 'CONFLICT',
    stateLabel: '冲突锁定',
    time: '07-21 13:26',
  },
  {
    batchId: 'A-20260721-007',
    bytes: '8.6 TB',
    key: 'A-20260721-007',
    media: 'SN …0D16',
    result: '上传完成 · 正在校验',
    site: 'A-002',
    state: 'VERIFYING',
    stateLabel: '目标校验',
    time: '07-21 10:31',
  },
  {
    batchId: 'A-20260720-019',
    bytes: '3.1 TB',
    key: 'A-20260720-019',
    media: 'SN …91A4',
    result: '来源签名被拒绝',
    site: 'A-004',
    state: 'FAILED',
    stateLabel: '失败',
    time: '07-20 18:42',
  },
];

export const historySourceOptions = [
  { label: '全部来源', value: 'ALL' },
  { label: 'A-001', value: 'A-001' },
  { label: 'A-002', value: 'A-002' },
  { label: 'A-003', value: 'A-003' },
  { label: 'A-004', value: 'A-004' },
  { label: 'A-006', value: 'A-006' },
] as const;

export const historyRangeOptions = [
  { label: '最近 7 天', value: '7' },
  { label: '最近 30 天', value: '30' },
  { label: '全部时间', value: 'ALL' },
] as const;
