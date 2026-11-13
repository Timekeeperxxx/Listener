#import <UIKit/UIKit.h>
#import <WebKit/WebKit.h>

typedef void (*ListenerLoginComplete)(void *, int, const uint8_t *, size_t);

@interface ListenerQQLogin : UIViewController <WKNavigationDelegate, WKUIDelegate>
@property (nonatomic, strong) NSURL *authorizationURL;
@property (nonatomic, strong) WKWebView *webView;
@property (nonatomic, assign) void *context;
@property (nonatomic, assign) ListenerLoginComplete complete;
@property (nonatomic, assign) BOOL waitingForQQ;
@property (nonatomic, assign) BOOL leftForQQ;
@property (nonatomic, assign) NSUInteger handoff;
@end

static __weak ListenerQQLogin *activeLogin;

@implementation ListenerQQLogin
- (void)loadView {
    WKWebViewConfiguration *config = [WKWebViewConfiguration new];
    config.websiteDataStore = WKWebsiteDataStore.nonPersistentDataStore;
    self.webView = [[WKWebView alloc] initWithFrame:CGRectZero configuration:config];
    self.webView.navigationDelegate = self;
    self.webView.UIDelegate = self;
    self.view = self.webView;
    self.title = @"QQ 音乐授权";
    self.navigationItem.leftBarButtonItem = [[UIBarButtonItem alloc]
        initWithBarButtonSystemItem:UIBarButtonSystemItemClose target:self action:@selector(cancel)];
    [self.webView loadRequest:[NSURLRequest requestWithURL:self.authorizationURL]];
    [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(sceneDidEnterBackground:)
        name:UISceneDidEnterBackgroundNotification object:nil];
    [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(sceneDidActivate:)
        name:UISceneDidActivateNotification object:nil];
}

- (void)sceneDidEnterBackground:(NSNotification *)note {
    if (note.object == self.view.window.windowScene && self.waitingForQQ) self.leftForQQ = YES;
}

- (void)sceneDidActivate:(NSNotification *)note {
    if (note.object != self.view.window.windowScene || !self.leftForQQ) return;
    self.leftForQQ = NO;
    NSUInteger handoff = self.handoff;
    // UIKit can deliver the return URL just after reactivating the scene.
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 2 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
        if (self.waitingForQQ && self.handoff == handoff) [self finish:4 callback:nil];
    });
}

- (void)finish:(int)status callback:(NSURL *)callback {
    ListenerLoginComplete complete = self.complete;
    if (!complete) return;
    self.complete = NULL;
    [self.webView stopLoading];
    NSData *data = [callback.absoluteString dataUsingEncoding:NSUTF8StringEncoding];
    complete(self.context, status, data.bytes, data.length);
    [self dismissViewControllerAnimated:YES completion:nil];
}

- (void)cancel {
    [self finish:1 callback:nil];
}

- (void)webView:(WKWebView *)view decidePolicyForNavigationAction:(WKNavigationAction *)action
    decisionHandler:(void (^)(WKNavigationActionPolicy))decision {
    NSURL *url = action.request.URL;
    NSString *host = url.host.lowercaseString;
    if (action.targetFrame.mainFrame && [url.scheme isEqualToString:@"https"]
        && [host isEqualToString:@"y.qq.com"] && [url.path isEqualToString:@"/portal/wx_redirect.html"]) {
        decision(WKNavigationActionPolicyCancel);
        [self finish:0 callback:url];
        return;
    }
    if ([url.scheme isEqualToString:@"wtloginmqq"] && [host isEqualToString:@"ptlogin"]
        && [url.path isEqualToString:@"/qlogin"]) {
        decision(WKNavigationActionPolicyCancel);
        NSURLComponents *handoff = [NSURLComponents componentsWithURL:url resolvingAgainstBaseURL:NO];
        NSMutableArray<NSURLQueryItem *> *params = [NSMutableArray new];
        for (NSURLQueryItem *item in handoff.percentEncodedQueryItems) {
            if (![item.name isEqualToString:@"schemacallback"]) [params addObject:item];
        }
        [params addObject:[NSURLQueryItem queryItemWithName:@"schemacallback"
                                                    value:@"listener-qq-login%3A%2F%2Freturn%3Furl%3D"]];
        handoff.percentEncodedQueryItems = params;
        self.handoff += 1;
        self.waitingForQQ = YES;
        self.leftForQQ = NO;
        [UIApplication.sharedApplication openURL:handoff.URL options:@{} completionHandler:^(BOOL opened) {
            if (!opened) [self finish:3 callback:nil];
        }];
        return;
    }
    BOOL allowed = [url.scheme isEqualToString:@"https"]
        && ([host hasSuffix:@".qq.com"] || [host isEqualToString:@"qq.com"]);
    decision(allowed || [url.absoluteString isEqualToString:@"about:blank"]
             ? WKNavigationActionPolicyAllow : WKNavigationActionPolicyCancel);
}

- (WKWebView *)webView:(WKWebView *)view createWebViewWithConfiguration:(WKWebViewConfiguration *)configuration
    forNavigationAction:(WKNavigationAction *)action windowFeatures:(WKWindowFeatures *)features {
    if (!action.targetFrame) [view loadRequest:action.request];
    return nil;
}

- (void)webView:(WKWebView *)view didFailProvisionalNavigation:(WKNavigation *)navigation
    withError:(NSError *)error {
    if (error.code != NSURLErrorCancelled) [self finish:2 callback:nil];
}
@end

void listener_present_qq_login(void *controller, const uint8_t *bytes, size_t length,
                               void *context, ListenerLoginComplete complete) {
    UIViewController *presenter = (__bridge UIViewController *)controller;
    if (!presenter || presenter.presentedViewController) {
        complete(context, 2, NULL, 0);
        return;
    }
    NSString *url = [[NSString alloc] initWithBytes:bytes length:length encoding:NSUTF8StringEncoding];
    ListenerQQLogin *login = [ListenerQQLogin new];
    login.authorizationURL = [NSURL URLWithString:url];
    login.context = context;
    login.complete = complete;
    activeLogin = login;
    UINavigationController *navigation = [[UINavigationController alloc] initWithRootViewController:login];
    navigation.modalPresentationStyle = UIModalPresentationFullScreen;
    [presenter presentViewController:navigation animated:YES completion:nil];
}

void listener_resume_qq_login(const uint8_t *bytes, size_t length) {
    ListenerQQLogin *login = activeLogin;
    if (!login.waitingForQQ || !login.complete) return;
    login.waitingForQQ = NO;
    NSString *text = [[NSString alloc] initWithBytes:bytes length:length encoding:NSUTF8StringEncoding];
    [login.webView loadRequest:[NSURLRequest requestWithURL:[NSURL URLWithString:text]]];
}
