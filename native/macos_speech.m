#import <AVFAudio/AVFAudio.h>
#import <AVFoundation/AVFoundation.h>
#import <Foundation/Foundation.h>
#import <Speech/Speech.h>
#import <objc/message.h>

typedef void (*RemSpeechCallback)(int event, const char *message, void *context);

typedef NS_ENUM(NSInteger, RemSpeechEvent) {
    RemSpeechEventAuthorizing = 0,
    RemSpeechEventRecording = 1,
    RemSpeechEventRecognizing = 2,
    RemSpeechEventPartial = 3,
    RemSpeechEventFinal = 4,
    RemSpeechEventPermissionDenied = 5,
    RemSpeechEventUnavailable = 6,
    RemSpeechEventError = 7,
};

@interface RemSpeechSession : NSObject

@property(nonatomic, assign) RemSpeechCallback callback;
@property(nonatomic, assign) void *context;
@property(nonatomic, strong) AVAudioEngine *audioEngine;
@property(nonatomic, strong) SFSpeechRecognizer *recognizer;
@property(nonatomic, strong) SFSpeechAudioBufferRecognitionRequest *request;
@property(nonatomic, strong) SFSpeechRecognitionTask *task;
@property(nonatomic, assign) BOOL cancelled;
@property(nonatomic, assign) BOOL tapInstalled;
@property(nonatomic, assign) NSUInteger authorizationAttempt;
@property(nonatomic, strong) dispatch_queue_t queue;

- (instancetype)initWithCallback:(RemSpeechCallback)callback context:(void *)context;
- (void)start;
- (void)stop;
- (void)cancel;

@end

@implementation RemSpeechSession

- (instancetype)initWithCallback:(RemSpeechCallback)callback context:(void *)context {
    self = [super init];
    if (self) {
        _callback = callback;
        _context = context;
        _queue = dispatch_queue_create("link.about-tttol.rem-cli.speech", DISPATCH_QUEUE_SERIAL);
    }
    return self;
}

- (void)sendEvent:(RemSpeechEvent)event message:(NSString *)message {
    const char *utf8 = message == nil ? NULL : message.UTF8String;
    self.callback((int)event, utf8, self.context);
}

- (void)start {
    dispatch_async(self.queue, ^{
        self.cancelled = NO;
        self.authorizationAttempt += 1;
        NSUInteger attempt = self.authorizationAttempt;
        NSOperatingSystemVersion minimum = {14, 0, 0};
        if (![[NSProcessInfo processInfo] isOperatingSystemAtLeastVersion:minimum]) {
            [self sendEvent:RemSpeechEventUnavailable
                    message:@"Voice input requires macOS 14 or later"];
            return;
        }

        [self sendEvent:RemSpeechEventAuthorizing message:nil];
        dispatch_after(
            dispatch_time(DISPATCH_TIME_NOW, 15 * NSEC_PER_SEC),
            self.queue,
            ^{
                if (!self.cancelled && self.authorizationAttempt == attempt &&
                    self.audioEngine == nil) {
                    self.cancelled = YES;
                    [self sendEvent:RemSpeechEventError
                            message:@"Timed out while waiting for macOS voice permissions"];
                }
            });
        [SFSpeechRecognizer requestAuthorization:^(SFSpeechRecognizerAuthorizationStatus status) {
            dispatch_async(self.queue, ^{
                if (self.cancelled || self.authorizationAttempt != attempt) {
                    return;
                }
                if (status != SFSpeechRecognizerAuthorizationStatusAuthorized) {
                    self.authorizationAttempt += 1;
                    [self sendEvent:RemSpeechEventPermissionDenied
                            message:@"Speech recognition permission was denied"];
                    return;
                }
                [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio
                                        completionHandler:^(BOOL granted) {
                    dispatch_async(self.queue, ^{
                        if (self.cancelled || self.authorizationAttempt != attempt) {
                            return;
                        }
                        if (!granted) {
                            self.authorizationAttempt += 1;
                            [self sendEvent:RemSpeechEventPermissionDenied
                                    message:@"Microphone permission was denied"];
                            return;
                        }
                        [self beginRecognition];
                    });
                }];
            });
        }];
    });
}

- (void)beginRecognition {
    if (self.cancelled) {
        return;
    }
    self.authorizationAttempt += 1;
    NSLocale *locale = [[NSLocale alloc] initWithLocaleIdentifier:@"ja-JP"];
    self.recognizer = [[SFSpeechRecognizer alloc] initWithLocale:locale];
    if (self.recognizer == nil || !self.recognizer.available) {
        [self sendEvent:RemSpeechEventUnavailable
                message:@"Japanese speech recognition is unavailable"];
        return;
    }

    SEL supportSelector = NSSelectorFromString(@"supportsOnDeviceRecognition");
    if (![self.recognizer respondsToSelector:supportSelector]) {
        [self sendEvent:RemSpeechEventUnavailable
                message:@"On-device speech recognition is unavailable"];
        return;
    }
    BOOL (*supportsOnDeviceRecognition)(id, SEL) = (void *)objc_msgSend;
    if (!supportsOnDeviceRecognition(self.recognizer, supportSelector)) {
        [self sendEvent:RemSpeechEventUnavailable
                message:@"On-device Japanese speech recognition is unavailable"];
        return;
    }

    self.request = [[SFSpeechAudioBufferRecognitionRequest alloc] init];
    self.request.requiresOnDeviceRecognition = YES;
    self.request.shouldReportPartialResults = YES;
    self.request.addsPunctuation = YES;
    self.request.taskHint = SFSpeechRecognitionTaskHintDictation;
    __weak RemSpeechSession *weakSelf = self;
    self.task = [self.recognizer
        recognitionTaskWithRequest:self.request
                     resultHandler:^(SFSpeechRecognitionResult *result, NSError *error) {
        RemSpeechSession *strongSelf = weakSelf;
        if (strongSelf == nil) {
            return;
        }
        if (result != nil) {
            NSString *text = result.bestTranscription.formattedString;
            [strongSelf sendEvent:(result.final ? RemSpeechEventFinal : RemSpeechEventPartial)
                          message:text];
            if (result.final) {
                [strongSelf cleanup];
                return;
            }
        }
        if (error != nil) {
            if (strongSelf.cancelled) {
                [strongSelf cleanup];
                return;
            }
            [strongSelf sendEvent:RemSpeechEventError message:error.localizedDescription];
            [strongSelf cleanup];
        }
    }];

    self.audioEngine = [[AVAudioEngine alloc] init];
    AVAudioInputNode *inputNode = self.audioEngine.inputNode;
    AVAudioFormat *format = [inputNode outputFormatForBus:0];
    if (format.channelCount == 0 || format.sampleRate == 0) {
        [self sendEvent:RemSpeechEventError message:@"No microphone input is available"];
        [self cleanup];
        return;
    }

    [inputNode installTapOnBus:0
                    bufferSize:1024
                        format:format
                         block:^(AVAudioPCMBuffer *buffer, AVAudioTime *when) {
        (void)when;
        RemSpeechSession *strongSelf = weakSelf;
        [strongSelf.request appendAudioPCMBuffer:buffer];
    }];
    self.tapInstalled = YES;
    [self.audioEngine prepare];

    NSError *startError = nil;
    if (![self.audioEngine startAndReturnError:&startError]) {
        [self sendEvent:RemSpeechEventError message:startError.localizedDescription];
        [self cleanup];
        return;
    }
    [self sendEvent:RemSpeechEventRecording message:nil];
}

- (void)stop {
    dispatch_async(self.queue, ^{
        if (self.audioEngine == nil || self.request == nil) {
            return;
        }
        [self.audioEngine stop];
        if (self.tapInstalled) {
            [self.audioEngine.inputNode removeTapOnBus:0];
            self.tapInstalled = NO;
        }
        [self.request endAudio];
        [self sendEvent:RemSpeechEventRecognizing message:nil];
    });
}

- (void)cancel {
    dispatch_async(self.queue, ^{
        self.cancelled = YES;
        self.authorizationAttempt += 1;
        [self.task cancel];
        [self cleanup];
    });
}

- (void)cleanup {
    if (self.audioEngine != nil) {
        [self.audioEngine stop];
        if (self.tapInstalled) {
            [self.audioEngine.inputNode removeTapOnBus:0];
        }
    }
    self.tapInstalled = NO;
    self.audioEngine = nil;
    self.request = nil;
    self.task = nil;
    self.recognizer = nil;
}

@end

void *rem_speech_create(RemSpeechCallback callback, void *context) {
    RemSpeechSession *session =
        [[RemSpeechSession alloc] initWithCallback:callback context:context];
    return (__bridge_retained void *)session;
}

void rem_speech_start(void *handle) {
    RemSpeechSession *session = (__bridge RemSpeechSession *)handle;
    [session start];
}

void rem_speech_stop(void *handle) {
    RemSpeechSession *session = (__bridge RemSpeechSession *)handle;
    [session stop];
}

void rem_speech_cancel(void *handle) {
    RemSpeechSession *session = (__bridge RemSpeechSession *)handle;
    [session cancel];
}

void rem_speech_destroy(void *handle) {
    RemSpeechSession *session = (__bridge_transfer RemSpeechSession *)handle;
    [session cancel];
}
