interface SpinnerOptions {
    failedText?: string;
    successText?: string;
    title: string;
}
export declare function spinner<T>({ failedText, successText, title }: SpinnerOptions, callback: () => Promise<T>): Promise<T>;
export {};
