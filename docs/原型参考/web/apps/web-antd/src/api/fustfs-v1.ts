import type { V1RequestConfig, V1Transport } from '#/generated/fustfs-v1';

import { useAppConfig } from '@vben/hooks';
import { defaultResponseInterceptor, RequestClient } from '@vben/request';

const { apiURL } = useAppConfig(import.meta.env, import.meta.env.PROD);

const fustfsV1RequestClient = new RequestClient({
  baseURL: apiURL,
  responseReturn: 'body',
});

fustfsV1RequestClient.addResponseInterceptor(
  defaultResponseInterceptor({
    codeField: 'code',
    dataField: 'data',
    successCode: 0,
  }),
);

export const fustfsV1Transport: V1Transport = async <T>(
  config: V1RequestConfig,
) =>
  fustfsV1RequestClient.request<T>(config.url, {
    data: config.data,
    headers: config.headers,
    method: config.method,
    params: config.params,
  });

export * from '#/generated/fustfs-v1';
