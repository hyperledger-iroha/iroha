#if os(iOS)
import UIKit

/// Full, selectable reviewed identities remain available even when the OS authentication
/// dialog shortens its reason. A background transition or cancellation closes this review.
@MainActor
final class KagemushaSpartanDebitTermsV1: UIViewController {
  private let terms: String
  private var decision: CheckedContinuation<Void, Error>?
  private var canceled = false
  init(terms: String) { self.terms = terms; super.init(nibName: nil, bundle: nil) }
  required init?(coder: NSCoder) { nil }

  func present(on host: UIViewController) async throws {
    guard !Task.isCancelled, host.viewIfLoaded?.window != nil, host.presentedViewController == nil,
      host.view.window?.windowScene?.activationState == .foregroundActive else { throw CancellationError() }
    try await withTaskCancellationHandler {
      try await withCheckedThrowingContinuation { (decision: CheckedContinuation<Void, Error>) in
        guard !canceled else { decision.resume(throwing: CancellationError()); return }
        self.decision = decision
        isModalInPresentation = true
        host.present(self, animated: true)
      }
    } onCancel: { Task { @MainActor in self.decline() } }
  }

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .systemBackground
    let title = UILabel()
    title.font = .preferredFont(forTextStyle: .title2)
    title.adjustsFontForContentSizeCategory = true
    title.text = "Review offline wallet debit"
    title.accessibilityTraits = .header
    title.numberOfLines = 0
    let body = UITextView()
    body.isEditable = false
    body.isSelectable = true
    body.font = .preferredFont(forTextStyle: .body)
    body.adjustsFontForContentSizeCategory = true
    body.text = terms
    let approve = UIButton(type: .system), cancel = UIButton(type: .system)
    approve.configuration = .filled(); approve.configuration?.title = "Continue"
    cancel.configuration = .plain(); cancel.configuration?.title = "Cancel"
    approve.addTarget(self, action: #selector(accept), for: .touchUpInside)
    cancel.addTarget(self, action: #selector(decline), for: .touchUpInside)
    let stack = UIStackView(arrangedSubviews: [title, body, approve, cancel])
    stack.axis = .vertical; stack.spacing = 12
    stack.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(stack)
    let guide = view.safeAreaLayoutGuide
    NSLayoutConstraint.activate([
      stack.topAnchor.constraint(equalTo: guide.topAnchor, constant: 22),
      stack.bottomAnchor.constraint(equalTo: guide.bottomAnchor, constant: -18),
      stack.leadingAnchor.constraint(equalTo: guide.leadingAnchor, constant: 20),
      stack.trailingAnchor.constraint(equalTo: guide.trailingAnchor, constant: -20),
    ])
    NotificationCenter.default.addObserver(self, selector: #selector(decline), name: UIApplication.didEnterBackgroundNotification, object: nil)
    NotificationCenter.default.addObserver(self, selector: #selector(decline), name: UIApplication.protectedDataWillBecomeUnavailableNotification, object: nil)
  }

  @objc private func accept() { finish(true) }
  @objc private func decline() { canceled = true; finish(false) }
  private func finish(_ approved: Bool) {
    guard let decision else { return }
    self.decision = nil
    NotificationCenter.default.removeObserver(self)
    dismiss(animated: false)
    if approved { decision.resume() } else { decision.resume(throwing: CancellationError()) }
  }
  override func viewDidDisappear(_ animated: Bool) {
    super.viewDidDisappear(animated)
    if let decision, presentingViewController == nil {
      self.decision = nil
      decision.resume(throwing: CancellationError())
    }
  }
}
#endif
